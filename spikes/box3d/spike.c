// W4a: Box3D repeatability and step cost. Build and run with ./run.sh
// Scene: a height field, a 1,000-box pile dropped onto it, and 20 four-wheel vehicles
// (chassis hull, sphere wheels on wheel joints with suspension and spin motors).
// 600 steps at 60 Hz with 4 sub-steps. Prints a hash of every body transform.
#include "box3d/box3d.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t fnv( uint64_t h, const void* p, size_t n )
{
	const uint8_t* b = p;
	for ( size_t i = 0; i < n; ++i ) h = ( h ^ b[i] ) * 0x100000001b3ull;
	return h;
}
static double now( void )
{
	struct timespec t;
	timespec_get( &t, TIME_UTC );
	return t.tv_sec + t.tv_nsec * 1e-9;
}

#define MAXB 2000
int main( int argc, char** argv )
{
	int steps = argc > 1 ? atoi( argv[1] ) : 600;
	b3WorldDef wd = b3DefaultWorldDef();
	b3WorldId world = b3CreateWorld( &wd );
	static b3BodyId bodies[MAXB];
	int n = 0;

	// Ground: 65 x 65 height field, 4 m cells, rolling hills.
	enum { N = 65 };
	static float heights[N * N];
	for ( int z = 0; z < N; ++z )
		for ( int x = 0; x < N; ++x ) heights[z * N + x] = 1.5f * sinf( 0.21f * x ) * cosf( 0.17f * z );
	b3HeightFieldDef hd = { 0 };
	hd.heights = heights;
	hd.scale = ( b3Vec3 ){ 4.0f, 1.0f, 4.0f };
	hd.countX = N;
	hd.countZ = N;
	hd.globalMinimumHeight = -2.0f;
	hd.globalMaximumHeight = 2.0f;
	b3HeightFieldData* hf = b3CreateHeightField( &hd );
	b3BodyDef bd = b3DefaultBodyDef();
	bd.position.x = -128.0f;
	bd.position.z = -128.0f;
	b3BodyId ground = b3CreateBody( world, &bd );
	b3ShapeDef sd = b3DefaultShapeDef();
	b3CreateHeightFieldShape( ground, &sd, hf );

	// 1,000 boxes, 10 x 10 x 10.
	b3BoxHull box = b3MakeBoxHull( 0.5f, 0.5f, 0.5f );
	for ( int y = 0; y < 10; ++y )
		for ( int z = 0; z < 10; ++z )
			for ( int x = 0; x < 10; ++x )
			{
				b3BodyDef d = b3DefaultBodyDef();
				d.type = b3_dynamicBody;
				d.position.x = -40.0f + 1.05f * x;
				d.position.y = 4.0f + 1.05f * y;
				d.position.z = 1.05f * z;
				bodies[n] = b3CreateBody( world, &d );
				b3CreateHullShape( bodies[n++], &sd, &box.base );
			}

	// 20 vehicles.
	b3BoxHull chassis = b3MakeBoxHull( 2.0f, 0.4f, 0.9f );
	for ( int v = 0; v < 20; ++v )
	{
		b3BodyDef d = b3DefaultBodyDef();
		d.type = b3_dynamicBody;
		d.position.x = 10.0f + 8.0f * ( v % 5 );
		d.position.y = 4.0f;
		d.position.z = -30.0f + 8.0f * ( v / 5 );
		b3BodyId car = bodies[n++] = b3CreateBody( world, &d );
		b3CreateHullShape( car, &sd, &chassis.base );
		for ( int w = 0; w < 4; ++w )
		{
			b3Vec3 local = { ( w & 1 ) ? 1.4f : -1.4f, -0.5f, ( w & 2 ) ? 1.0f : -1.0f };
			b3BodyDef wdf = b3DefaultBodyDef();
			wdf.type = b3_dynamicBody;
			wdf.position.x = d.position.x + local.x;
			wdf.position.y = d.position.y + local.y;
			wdf.position.z = d.position.z + local.z;
			b3BodyId wheel = bodies[n++] = b3CreateBody( world, &wdf );
			b3Sphere s = { { 0.0f, 0.0f, 0.0f }, 0.4f };
			b3CreateSphereShape( wheel, &sd, &s );
			b3WheelJointDef jd = b3DefaultWheelJointDef();
			jd.base.bodyIdA = car;
			jd.base.bodyIdB = wheel;
			jd.base.localFrameA.p = local;
			jd.base.localFrameA.q = b3ComputeQuatBetweenUnitVectors( b3Vec3_axisX, b3Vec3_axisY );
			jd.base.localFrameB.q = b3ComputeQuatBetweenUnitVectors( b3Vec3_axisZ, b3Vec3_axisY );
			jd.enableSuspensionSpring = true;
			jd.suspensionHertz = 4.0f;
			jd.suspensionDampingRatio = 0.7f;
			jd.enableSuspensionLimit = true;
			jd.lowerSuspensionLimit = -0.25f;
			jd.upperSuspensionLimit = 0.25f;
			jd.enableSpinMotor = true;
			jd.maxSpinTorque = 400.0f;
			jd.spinSpeed = 6.0f + 0.5f * v;
			b3CreateWheelJoint( world, &jd );
		}
	}

	double total = 0.0, worst = 0.0;
	uint64_t h = 0xcbf29ce484222325ull;
	for ( int i = 0; i < steps; ++i )
	{
		double t0 = now();
		b3World_Step( world, 1.0f / 60.0f, 4 );
		double dt = now() - t0;
		total += dt;
		if ( dt > worst ) worst = dt;
		if ( ( i + 1 ) % 100 == 0 || i + 1 == steps )
		{
			for ( int b = 0; b < n; ++b )
			{
				b3WorldTransform t = b3Body_GetTransform( bodies[b] );
				h = fnv( h, &t.p, sizeof t.p );
				h = fnv( h, &t.q, sizeof t.q );
			}
			printf( "step %4d hash %016llx awake %d\n", i + 1, (unsigned long long)h, b3World_GetAwakeBodyCount( world ) );
		}
	}
	printf( "bodies %d  mean step %.3f ms  worst step %.3f ms  sizeof(position) %zu\n", n, 1e3 * total / steps, 1e3 * worst,
			sizeof( ( (b3WorldTransform*)0 )->p ) );
	b3DestroyWorld( world );
	return 0;
}
