// ed_sim's C shim over the vendored Box3D. Everything Rust calls goes through the functions
// in this file, with flat structs defined here (mirrored in crates/ed_sim/src/ffi.rs and checked
// by ed_b3_layout). Box3D definition structs (b3WorldDef, b3BodyDef, b3ShapeDef, ...) never cross
// the boundary: the shim fills them from Box3D's own defaults, so an upstream field change is a
// compile error here instead of silent memory corruption in Rust.
//
// Ids: Rust stores body and shape ids as a 64-bit key (index1 << 32 | generation). The world part
// is taken from the world id passed alongside, so ids stay valid when a snapshot taken in one
// world is restored into another.
//
// Snapshot and restore use Box3D's internal world serializer (world_snapshot.c), the same code
// the upstream replay player uses for its keyframes.

#include "box3d/box3d.h"
#include "box3d/collision.h"
#include "box3d/math_functions.h"

#include "core.h"
#include "physics_world.h"
#include "recording.h"
#include "replay.h"
#include "world_snapshot.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#if !defined( BOX3D_DOUBLE_PRECISION )
#error "ed_sim requires BOX3D_DOUBLE_PRECISION"
#endif

// ---------------------------------------------------------------------------------------------
// Boundary structs. Keep in sync with ffi.rs (the layout guard test enforces it).

typedef struct EdWorldDesc
{
	b3Vec3 gravity;
	uint8_t enableSleep;
	uint8_t enableContinuous;
	uint8_t pad[2];
} EdWorldDesc;

typedef struct EdBodyDesc
{
	b3Pos position;
	b3Quat rotation;
	b3Vec3 linearVelocity;
	b3Vec3 angularVelocity;
	float linearDamping;
	float angularDamping;
	float gravityScale;
	int32_t type; // 0 static, 1 kinematic, 2 dynamic (b3BodyType)
	uint8_t isBullet;
	uint8_t enableSleep;
	uint8_t lockRotation;
	uint8_t pad;
} EdBodyDesc;

typedef struct EdShapeMat
{
	uint64_t categoryBits;
	uint64_t maskBits;
	float density;
	float friction;
	float restitution;
	float pad;
} EdShapeMat;

typedef struct EdBodyState
{
	b3Pos p;
	b3Quat q;
	b3Vec3 v;
	b3Vec3 w;
} EdBodyState;

typedef struct EdHit
{
	b3Pos point;
	b3Vec3 normal;
	float fraction;
	uint64_t bodyKey;
	uint64_t shapeKey;
	int32_t hit;
	int32_t pad;
} EdHit;

typedef struct EdPlane
{
	b3Vec3 normal;
	float offset;
	b3Vec3 point;
	float pad;
} EdPlane;

// ---------------------------------------------------------------------------------------------
// Ids

static b3WorldId ed_world( uint32_t w )
{
	return b3LoadWorldId( w );
}

static b3BodyId ed_body( uint32_t w, uint64_t key )
{
	b3BodyId id;
	id.index1 = (int32_t)( key >> 32 );
	id.world0 = (uint16_t)( b3LoadWorldId( w ).index1 - 1 );
	id.generation = (uint16_t)( key & 0xFFFFu );
	return id;
}

static uint64_t ed_body_key( b3BodyId id )
{
	return ( (uint64_t)(uint32_t)id.index1 << 32 ) | (uint64_t)id.generation;
}

static uint64_t ed_shape_key( b3ShapeId id )
{
	return ( (uint64_t)(uint32_t)id.index1 << 32 ) | (uint64_t)id.generation;
}

// ---------------------------------------------------------------------------------------------
// Restored geometry. A restored mesh, height field or compound shape points into the snapshot's
// geometry bytes, so the shim keeps a copy per world until the next restore or the world's
// destruction. Each world index touches only its own entry.

typedef struct EdRestoreArena
{
	b3RegistrySlot* slots;
	int count;
} EdRestoreArena;

static EdRestoreArena ed_arenas[B3_MAX_WORLDS];

static void ed_free_arena( EdRestoreArena* a )
{
	for ( int i = 0; i < a->count; ++i )
	{
		if ( a->slots[i].bytes != NULL )
		{
			b3Free( a->slots[i].bytes, (size_t)a->slots[i].byteCount );
		}
	}
	if ( a->slots != NULL )
	{
		b3Free( a->slots, (size_t)a->count * sizeof( b3RegistrySlot ) );
	}
	a->slots = NULL;
	a->count = 0;
}

// ---------------------------------------------------------------------------------------------
// Info and layout guard

int ed_b3_is_double_precision( void )
{
	return b3IsDoublePrecision() ? 1 : 0;
}

#define ED_LAYOUT( T ) out[n++] = sizeof( T ); out[n++] = _Alignof( T );
#define ED_OFFSET( T, f ) out[n++] = offsetof( T, f );

// Writes sizes, alignments and field offsets in a fixed order (see ffi.rs `layout_guard`).
int ed_b3_layout( uint64_t* out, int capacity )
{
	int n = 0;
	if ( capacity < 128 )
	{
		return -1;
	}
	ED_LAYOUT( b3Vec3 )
	ED_LAYOUT( b3Quat )
	ED_LAYOUT( b3Pos )
	ED_LAYOUT( b3Capsule )
	ED_OFFSET( b3Capsule, center2 )
	ED_OFFSET( b3Capsule, radius )
	ED_LAYOUT( EdWorldDesc )
	ED_OFFSET( EdWorldDesc, enableSleep )
	ED_OFFSET( EdWorldDesc, enableContinuous )
	ED_LAYOUT( EdBodyDesc )
	ED_OFFSET( EdBodyDesc, rotation )
	ED_OFFSET( EdBodyDesc, linearVelocity )
	ED_OFFSET( EdBodyDesc, angularVelocity )
	ED_OFFSET( EdBodyDesc, linearDamping )
	ED_OFFSET( EdBodyDesc, gravityScale )
	ED_OFFSET( EdBodyDesc, type )
	ED_OFFSET( EdBodyDesc, isBullet )
	ED_OFFSET( EdBodyDesc, lockRotation )
	ED_LAYOUT( EdShapeMat )
	ED_OFFSET( EdShapeMat, maskBits )
	ED_OFFSET( EdShapeMat, density )
	ED_OFFSET( EdShapeMat, restitution )
	ED_LAYOUT( EdBodyState )
	ED_OFFSET( EdBodyState, q )
	ED_OFFSET( EdBodyState, v )
	ED_OFFSET( EdBodyState, w )
	ED_LAYOUT( EdHit )
	ED_OFFSET( EdHit, normal )
	ED_OFFSET( EdHit, fraction )
	ED_OFFSET( EdHit, bodyKey )
	ED_OFFSET( EdHit, shapeKey )
	ED_OFFSET( EdHit, hit )
	ED_LAYOUT( EdPlane )
	ED_OFFSET( EdPlane, offset )
	ED_OFFSET( EdPlane, point )
	return n;
}

// ---------------------------------------------------------------------------------------------
// World

uint32_t ed_world_create( const EdWorldDesc* desc )
{
	b3WorldDef def = b3DefaultWorldDef();
	def.gravity = desc->gravity;
	def.enableSleep = desc->enableSleep != 0;
	def.enableContinuous = desc->enableContinuous != 0;
	// One thread, no task system: the step runs serially and deterministically.
	def.workerCount = 1;
	def.enqueueTask = NULL;
	def.finishTask = NULL;
	b3WorldId id = b3CreateWorld( &def );
	if ( B3_IS_NULL( id ) )
	{
		return 0;
	}
	ed_free_arena( &ed_arenas[id.index1 - 1] );
	return b3StoreWorldId( id );
}

void ed_world_destroy( uint32_t w )
{
	b3WorldId id = ed_world( w );
	b3DestroyWorld( id );
	ed_free_arena( &ed_arenas[id.index1 - 1] );
}

void ed_world_step( uint32_t w, float dt, int substeps )
{
	b3World_Step( ed_world( w ), dt, substeps );
}

int ed_world_awake_body_count( uint32_t w )
{
	return b3World_GetAwakeBodyCount( ed_world( w ) );
}

// ---------------------------------------------------------------------------------------------
// Bodies

uint64_t ed_body_create( uint32_t w, const EdBodyDesc* desc )
{
	b3BodyDef def = b3DefaultBodyDef();
	def.type = (b3BodyType)desc->type;
	def.position = desc->position;
	def.rotation = desc->rotation;
	def.linearVelocity = desc->linearVelocity;
	def.angularVelocity = desc->angularVelocity;
	def.linearDamping = desc->linearDamping;
	def.angularDamping = desc->angularDamping;
	def.gravityScale = desc->gravityScale;
	def.isBullet = desc->isBullet != 0;
	def.enableSleep = desc->enableSleep != 0;
	if ( desc->lockRotation )
	{
		def.motionLocks.angularX = true;
		def.motionLocks.angularY = true;
		def.motionLocks.angularZ = true;
	}
	b3BodyId id = b3CreateBody( ed_world( w ), &def );
	return ed_body_key( id );
}

void ed_body_destroy( uint32_t w, uint64_t key )
{
	b3DestroyBody( ed_body( w, key ) );
}

void ed_bodies_read( uint32_t w, const uint64_t* keys, int count, EdBodyState* out )
{
	for ( int i = 0; i < count; ++i )
	{
		b3BodyId id = ed_body( w, keys[i] );
		b3WorldTransform t = b3Body_GetTransform( id );
		out[i].p = t.p;
		out[i].q = t.q;
		out[i].v = b3Body_GetLinearVelocity( id );
		out[i].w = b3Body_GetAngularVelocity( id );
	}
}

int ed_body_is_awake( uint32_t w, uint64_t key )
{
	return b3Body_IsAwake( ed_body( w, key ) ) ? 1 : 0;
}

void ed_body_set_transform( uint32_t w, uint64_t key, const b3Pos* p, const b3Quat* q )
{
	b3Body_SetTransform( ed_body( w, key ), *p, *q );
}

void ed_body_set_velocity( uint32_t w, uint64_t key, const b3Vec3* v, const b3Vec3* av )
{
	b3BodyId id = ed_body( w, key );
	b3Body_SetLinearVelocity( id, *v );
	b3Body_SetAngularVelocity( id, *av );
}

void ed_body_set_target( uint32_t w, uint64_t key, const b3Pos* p, const b3Quat* q, float dt )
{
	b3WorldTransform t = { *p, *q };
	b3Body_SetTargetTransform( ed_body( w, key ), t, dt, true );
}

void ed_body_apply_impulse( uint32_t w, uint64_t key, const b3Vec3* impulse )
{
	b3Body_ApplyLinearImpulseToCenter( ed_body( w, key ), *impulse, true );
}

// ---------------------------------------------------------------------------------------------
// Shapes

static b3ShapeDef ed_shape_def( const EdShapeMat* m )
{
	b3ShapeDef def = b3DefaultShapeDef();
	def.density = m->density;
	def.baseMaterial.friction = m->friction;
	def.baseMaterial.restitution = m->restitution;
	def.filter.categoryBits = m->categoryBits;
	def.filter.maskBits = m->maskBits;
	return def;
}

uint64_t ed_shape_box( uint32_t w, uint64_t body, const EdShapeMat* m, const b3Vec3* half, const b3Vec3* p,
					   const b3Quat* q )
{
	b3ShapeDef def = ed_shape_def( m );
	b3Transform t = { *p, *q };
	b3BoxHull box = b3MakeTransformedBoxHull( half->x, half->y, half->z, t );
	return ed_shape_key( b3CreateHullShape( ed_body( w, body ), &def, &box.base ) );
}

uint64_t ed_shape_sphere( uint32_t w, uint64_t body, const EdShapeMat* m, const b3Vec3* center, float radius )
{
	b3ShapeDef def = ed_shape_def( m );
	b3Sphere s = { *center, radius };
	return ed_shape_key( b3CreateSphereShape( ed_body( w, body ), &def, &s ) );
}

uint64_t ed_shape_capsule( uint32_t w, uint64_t body, const EdShapeMat* m, const b3Capsule* c )
{
	b3ShapeDef def = ed_shape_def( m );
	return ed_shape_key( b3CreateCapsuleShape( ed_body( w, body ), &def, c ) );
}

uint64_t ed_shape_hull( uint32_t w, uint64_t body, const EdShapeMat* m, const void* hull, const b3Vec3* p,
						const b3Quat* q )
{
	b3ShapeDef def = ed_shape_def( m );
	b3Transform t = { *p, *q };
	b3Vec3 one = { 1.0f, 1.0f, 1.0f };
	return ed_shape_key(
		b3CreateTransformedHullShape( ed_body( w, body ), &def, (const b3HullData*)hull, t, one ) );
}

uint64_t ed_shape_mesh( uint32_t w, uint64_t body, const EdShapeMat* m, const void* mesh )
{
	b3ShapeDef def = ed_shape_def( m );
	b3Vec3 one = { 1.0f, 1.0f, 1.0f };
	return ed_shape_key( b3CreateMeshShape( ed_body( w, body ), &def, (const b3MeshData*)mesh, one ) );
}

uint64_t ed_shape_height_field( uint32_t w, uint64_t body, const EdShapeMat* m, const void* hf )
{
	b3ShapeDef def = ed_shape_def( m );
	return ed_shape_key( b3CreateHeightFieldShape( ed_body( w, body ), &def, (const b3HeightFieldData*)hf ) );
}

// ---------------------------------------------------------------------------------------------
// Geometry (owned by Rust through these handles)

void* ed_hull_create( const b3Vec3* points, int count )
{
	return b3CreateHull( points, count, B3_MAX_HULL_VERTICES );
}

void ed_hull_destroy( void* hull )
{
	b3DestroyHull( (b3HullData*)hull );
}

// Writes the hull's unique edges as point pairs (6 floats per edge). Returns the edge count.
int ed_hull_edges( const void* hullPtr, float* out, int edgeCapacity )
{
	const b3HullData* hull = (const b3HullData*)hullPtr;
	const b3Vec3* points = b3GetHullPoints( hull );
	const b3HullHalfEdge* edges = b3GetHullEdges( hull );
	int n = 0;
	for ( int i = 0; i < hull->edgeCount; ++i )
	{
		const b3HullHalfEdge* e = edges + i;
		if ( e->twin < i )
		{
			continue;
		}
		if ( n >= edgeCapacity )
		{
			break;
		}
		b3Vec3 a = points[e->origin];
		b3Vec3 b = points[edges[e->twin].origin];
		float* o = out + 6 * n;
		o[0] = a.x;
		o[1] = a.y;
		o[2] = a.z;
		o[3] = b.x;
		o[4] = b.y;
		o[5] = b.z;
		n += 1;
	}
	return n;
}

void* ed_mesh_create( const b3Vec3* vertices, int vertexCount, const int32_t* indices, int triangleCount )
{
	b3MeshDef def = { 0 };
	def.vertices = (b3Vec3*)vertices;
	def.stride = sizeof( b3Vec3 );
	def.indices = (int32_t*)indices;
	def.vertexCount = vertexCount;
	def.triangleCount = triangleCount;
	def.weldVertices = false;
	def.identifyEdges = true;
	def.clockWiseWinding = false;
	return b3CreateMesh( &def, NULL, 0 );
}

void ed_mesh_destroy( void* mesh )
{
	b3DestroyMesh( (b3MeshData*)mesh );
}

void* ed_height_field_create( const float* heights, int countX, int countZ, const b3Vec3* scale, float minHeight,
							  float maxHeight )
{
	b3HeightFieldDef def = { 0 };
	def.heights = (float*)heights;
	def.materialIndices = NULL;
	def.scale = *scale;
	def.countX = countX;
	def.countZ = countZ;
	def.globalMinimumHeight = minHeight;
	def.globalMaximumHeight = maxHeight;
	def.clockwiseWinding = false;
	return b3CreateHeightField( &def );
}

void ed_height_field_destroy( void* hf )
{
	b3DestroyHeightField( (b3HeightFieldData*)hf );
}

// ---------------------------------------------------------------------------------------------
// Queries

static b3QueryFilter ed_filter( uint64_t category, uint64_t mask )
{
	b3QueryFilter f = b3DefaultQueryFilter();
	f.categoryBits = category;
	f.maskBits = mask;
	return f;
}

static void ed_fill_hit( EdHit* out, b3ShapeId shape, b3Pos point, b3Vec3 normal, float fraction )
{
	out->point = point;
	out->normal = normal;
	out->fraction = fraction;
	out->shapeKey = ed_shape_key( shape );
	out->bodyKey = ed_body_key( b3Shape_GetBody( shape ) );
	out->hit = 1;
}

int ed_cast_ray( uint32_t w, const b3Pos* origin, const b3Vec3* translation, uint64_t category, uint64_t mask,
				 EdHit* out )
{
	memset( out, 0, sizeof( *out ) );
	b3RayResult r = b3World_CastRayClosest( ed_world( w ), *origin, *translation, ed_filter( category, mask ) );
	if ( r.hit == false )
	{
		return 0;
	}
	ed_fill_hit( out, r.shapeId, r.point, r.normal, r.fraction );
	return 1;
}

// Closest hit. Ties keep the first reported shape (the tree traversal order is deterministic).
static float ed_closest_cast_fcn( b3ShapeId shapeId, b3Pos point, b3Vec3 normal, float fraction, uint64_t userMaterialId,
								  int triangleIndex, int childIndex, void* context )
{
	(void)userMaterialId;
	(void)triangleIndex;
	(void)childIndex;
	EdHit* out = (EdHit*)context;
	if ( out->hit == 0 || fraction < out->fraction )
	{
		ed_fill_hit( out, shapeId, point, normal, fraction );
	}
	return fraction;
}

int ed_cast_shape( uint32_t w, const b3Pos* origin, const b3Vec3* points, int count, float radius,
				   const b3Vec3* translation, uint64_t category, uint64_t mask, EdHit* out )
{
	memset( out, 0, sizeof( *out ) );
	b3ShapeProxy proxy = { points, count, radius };
	b3World_CastShape( ed_world( w ), *origin, &proxy, *translation, ed_filter( category, mask ), ed_closest_cast_fcn,
					   out );
	return out->hit;
}

typedef struct EdPlaneSink
{
	EdPlane* planes;
	int count;
	int capacity;
} EdPlaneSink;

static bool ed_plane_fcn( b3ShapeId shapeId, const b3PlaneResult* planes, int planeCount, void* context )
{
	(void)shapeId;
	EdPlaneSink* sink = (EdPlaneSink*)context;
	for ( int i = 0; i < planeCount && sink->count < sink->capacity; ++i )
	{
		EdPlane* p = sink->planes + sink->count;
		p->normal = planes[i].plane.normal;
		p->offset = planes[i].plane.offset;
		p->point = planes[i].point;
		p->pad = 0.0f;
		sink->count += 1;
	}
	return sink->count < sink->capacity;
}

int ed_collide_mover( uint32_t w, const b3Pos* origin, const b3Capsule* mover, uint64_t category, uint64_t mask,
					  EdPlane* out, int capacity )
{
	EdPlaneSink sink = { out, 0, capacity };
	b3World_CollideMover( ed_world( w ), *origin, mover, ed_filter( category, mask ), ed_plane_fcn, &sink );
	return sink.count;
}

float ed_cast_mover( uint32_t w, const b3Pos* origin, const b3Capsule* mover, const b3Vec3* translation,
					 uint64_t category, uint64_t mask )
{
	return b3World_CastMover( ed_world( w ), *origin, mover, *translation, ed_filter( category, mask ), NULL, NULL );
}

// ---------------------------------------------------------------------------------------------
// Snapshot and restore
//
// Blob layout (little-endian on every supported target; Box3D's image itself is a same-build,
// same-architecture format): u32 imageSize, image bytes, u32 slotCount, then per slot
// u32 kind, u32 byteCount, bytes.

static void ed_put_u32( uint8_t* dst, uint32_t v )
{
	dst[0] = (uint8_t)v;
	dst[1] = (uint8_t)( v >> 8 );
	dst[2] = (uint8_t)( v >> 16 );
	dst[3] = (uint8_t)( v >> 24 );
}

static uint32_t ed_get_u32( const uint8_t* src )
{
	return (uint32_t)src[0] | ( (uint32_t)src[1] << 8 ) | ( (uint32_t)src[2] << 16 ) | ( (uint32_t)src[3] << 24 );
}

// Returns a b3Alloc'd blob (free with ed_blob_free) and its size, or NULL.
uint8_t* ed_world_snapshot( uint32_t w, int* outSize )
{
	*outSize = 0;
	b3World* world = b3GetWorldFromId( ed_world( w ) );
	b3Recording* rec = b3CreateRecording( 0 );
	b3RecBuffer buf = { 0 };
	b3SerializeWorld( world, &buf, rec );

	b3GeometryRegistry* reg = &rec->registry;
	size_t total = 4 + (size_t)buf.size + 4;
	for ( int i = 0; i < reg->entries.count; ++i )
	{
		total += 8 + (size_t)reg->entries.data[i].byteCount;
	}

	uint8_t* blob = (uint8_t*)b3Alloc( total );
	uint8_t* p = blob;
	ed_put_u32( p, (uint32_t)buf.size );
	p += 4;
	memcpy( p, buf.data, (size_t)buf.size );
	p += buf.size;
	ed_put_u32( p, (uint32_t)reg->entries.count );
	p += 4;
	for ( int i = 0; i < reg->entries.count; ++i )
	{
		b3GeometryEntry* e = reg->entries.data + i;
		ed_put_u32( p, (uint32_t)e->kind );
		ed_put_u32( p + 4, (uint32_t)e->byteCount );
		p += 8;
		memcpy( p, e->bytes, (size_t)e->byteCount );
		p += e->byteCount;
	}

	b3RecBufFree( &buf );
	b3DestroyRecording( rec );
	*outSize = (int)total;
	return blob;
}

void ed_blob_free( uint8_t* blob, int size )
{
	b3Free( blob, (size_t)size );
}

// Overwrites the live world with the snapshot. Returns 1 on success, 0 if the blob is malformed
// or from an incompatible build (checked before the world is touched), -1 if Box3D rejected the
// image after it started writing (the world is then unusable and must be destroyed).
int ed_world_restore( uint32_t w, const uint8_t* blob, int size )
{
	if ( size < 8 )
	{
		return 0;
	}
	uint32_t imageSize = ed_get_u32( blob );
	if ( (uint64_t)imageSize + 8 > (uint64_t)size )
	{
		return 0;
	}
	const uint8_t* image = blob + 4;
	const uint8_t* p = image + imageSize;
	const uint8_t* end = blob + size;
	uint32_t slotCount = ed_get_u32( p );
	p += 4;

	// Validate the slot table before allocating.
	{
		const uint8_t* q = p;
		for ( uint32_t i = 0; i < slotCount; ++i )
		{
			if ( end - q < 8 )
			{
				return 0;
			}
			uint32_t bc = ed_get_u32( q + 4 );
			q += 8;
			if ( (uint64_t)( end - q ) < bc )
			{
				return 0;
			}
			q += bc;
		}
		if ( q != end )
		{
			return 0;
		}
	}

	EdRestoreArena next = { 0 };
	next.count = (int)slotCount;
	if ( slotCount > 0 )
	{
		next.slots = (b3RegistrySlot*)b3Alloc( (size_t)slotCount * sizeof( b3RegistrySlot ) );
		memset( next.slots, 0, (size_t)slotCount * sizeof( b3RegistrySlot ) );
	}
	for ( uint32_t i = 0; i < slotCount; ++i )
	{
		uint32_t kind = ed_get_u32( p );
		uint32_t bc = ed_get_u32( p + 4 );
		p += 8;
		next.slots[i].kind = (b3GeometryKind)kind;
		next.slots[i].byteCount = (int)bc;
		next.slots[i].bytes = (uint8_t*)b3Alloc( bc > 0 ? bc : 1 );
		next.slots[i].byteCount = (int)( bc > 0 ? bc : 1 );
		memcpy( next.slots[i].bytes, p, bc );
		p += bc;
	}

	b3WorldId id = ed_world( w );
	b3World* world = b3GetWorldFromId( id );
	b3RecReader rdr;
	memset( &rdr, 0, sizeof( rdr ) );
	rdr.replayWorldId = id;
	rdr.ok = true;
	rdr.slots = next.slots;
	rdr.slotCount = next.count;

	bool ok = b3DeserializeIntoShell( image, (int)imageSize, world, &rdr );

	// Shapes now point into `next`; release the previous restore's copies.
	EdRestoreArena* arena = &ed_arenas[id.index1 - 1];
	ed_free_arena( arena );
	*arena = next;
	return ok ? 1 : -1;
}
