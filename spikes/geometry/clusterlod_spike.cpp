// W4b: build a cluster LOD hierarchy with meshoptimizer's clusterlod.h and report size,
// time and repeatability. Build and run: ./run.sh
#include "meshoptimizer/src/meshoptimizer.h"
#define CLUSTERLOD_IMPLEMENTATION
#include "meshoptimizer/demo/clusterlod.h"
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <vector>

struct Out { uint64_t hash = 0xcbf29ce484222325ull; size_t groups = 0, clusters = 0, tris = 0, lod0 = 0; int depth = 0; std::vector<size_t> per_depth; };
static void fnv(uint64_t& h, const void* p, size_t n) { auto b = (const unsigned char*)p; for (size_t i = 0; i < n; ++i) h = (h ^ b[i]) * 0x100000001b3ull; }

int main(int argc, char** argv)
{
	for (int a = 1; a < argc; ++a)
	{
		FILE* f = fopen(argv[a], "rb");
		if (!f) return 1;
		uint32_t nv, ni;
		fread(&nv, 4, 1, f); fread(&ni, 4, 1, f);
		std::vector<float> pos(nv * 3); std::vector<unsigned> idx(ni);
		fread(pos.data(), 4, pos.size(), f); fread(idx.data(), 4, ni, f); fclose(f);
		// Weld by position, as a bake would, so clusters can share boundaries.
		std::vector<unsigned> remap(nv);
		size_t wv = meshopt_generateVertexRemap(remap.data(), idx.data(), ni, pos.data(), nv, 12);
		meshopt_remapIndexBuffer(idx.data(), idx.data(), ni, remap.data());
		std::vector<float> wpos(wv * 3);
		meshopt_remapVertexBuffer(wpos.data(), pos.data(), nv, 12, remap.data());

		uint64_t first = 0;
		for (int run = 0; run < 2; ++run)
		{
			Out o;
			clodConfig cfg = clodDefaultConfig(128);
			clodMesh mesh = {};
			mesh.indices = idx.data(); mesh.index_count = ni; mesh.vertex_count = wv;
			mesh.vertex_positions = wpos.data(); mesh.vertex_positions_stride = 12;
			auto t0 = std::chrono::steady_clock::now();
			clodBuild(cfg, mesh, &o, [](void* ctx, clodGroup g, const clodCluster* c, size_t n) -> int {
				Out& o = *(Out*)ctx;
				o.groups++; if (g.depth > o.depth) o.depth = g.depth;
				if ((size_t)g.depth >= o.per_depth.size()) o.per_depth.resize(g.depth + 1);
				fnv(o.hash, &g.depth, sizeof g.depth); fnv(o.hash, &g.simplified, sizeof g.simplified);
				for (size_t i = 0; i < n; ++i)
				{
					o.clusters++; o.tris += c[i].index_count / 3; o.per_depth[g.depth] += c[i].index_count / 3;
					if (g.depth == 0) o.lod0 += c[i].index_count / 3;
					fnv(o.hash, c[i].indices, c[i].index_count * 4); fnv(o.hash, &c[i].bounds, sizeof c[i].bounds);
				}
				return (int)o.groups - 1;
			});
			double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
			if (run == 0)
			{
				first = o.hash;
				printf("%s: %u tris in, %zu welded vertices\n  %zu clusters in %zu groups, %d levels, %zu triangles stored (%.2fx the source), bake %.0f ms (%.2f M tris/s)\n  triangles per level:",
					argv[a], ni / 3, wv, o.clusters, o.groups, o.depth + 1, o.tris, double(o.tris) / (ni / 3), ms, (ni / 3) / ms / 1000.0);
				for (size_t d : o.per_depth) printf(" %zu", d);
				printf("\n");
			}
			else
				printf("  second bake: hash %s\n", o.hash == first ? "identical" : "DIFFERENT");
		}
	}
}
