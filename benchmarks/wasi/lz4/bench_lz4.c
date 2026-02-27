/*
 * LZ4 compression/decompression benchmark for WASI
 * Compresses and decompresses a buffer repeatedly, reports throughput.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "lz4.h"

#define DATA_SIZE (1024 * 1024)  /* 1 MB input buffer */
#define MIN_ITERATIONS 50
#define MIN_SECONDS 10.0

/* Generate pseudo-random but compressible data (English-like text patterns) */
static void generate_data(unsigned char *buf, int size) {
    /* Simulate text-like data with repeated words and patterns */
    const char *words[] = {
        "the ", "quick ", "brown ", "fox ", "jumps ", "over ", "lazy ", "dog ",
        "hello ", "world ", "benchmark ", "compression ", "data ", "test ",
        "performance ", "wasm ", "runtime ", "function ", "value ", "return ",
    };
    int nwords = 20;
    unsigned int state = 0xCAFEBABE;
    int pos = 0;
    while (pos < size) {
        state = state * 1103515245 + 12345;
        const char *w = words[(state >> 16) % nwords];
        while (*w && pos < size) {
            buf[pos++] = *w++;
        }
    }
}

int main(void) {
    unsigned char *input = malloc(DATA_SIZE);
    int max_compressed = LZ4_compressBound(DATA_SIZE);
    char *compressed = malloc(max_compressed);
    char *decompressed = malloc(DATA_SIZE);

    if (!input || !compressed || !decompressed) {
        fprintf(stderr, "Failed to allocate memory\n");
        return 1;
    }

    generate_data(input, DATA_SIZE);

    /* Warm up and verify */
    int comp_size = LZ4_compress_default((char *)input, compressed, DATA_SIZE, max_compressed);
    if (comp_size <= 0) {
        fprintf(stderr, "LZ4 compress failed\n");
        return 1;
    }

    int decomp_size = LZ4_decompress_safe(compressed, decompressed, comp_size, DATA_SIZE);
    if (decomp_size != DATA_SIZE) {
        fprintf(stderr, "LZ4 decompress failed: got %d expected %d\n", decomp_size, DATA_SIZE);
        return 1;
    }
    if (memcmp(input, decompressed, DATA_SIZE) != 0) {
        fprintf(stderr, "LZ4 roundtrip mismatch!\n");
        return 1;
    }

    printf("lz4 benchmark: %d KB input -> %d KB compressed (%.1fx)\n",
           DATA_SIZE / 1024, comp_size / 1024, (double)DATA_SIZE / comp_size);

    /* Benchmark compression */
    int iterations = 0;
    long long total_bytes = 0;
    clock_t start = clock();
    double elapsed;

    do {
        comp_size = LZ4_compress_default((char *)input, compressed, DATA_SIZE, max_compressed);
        iterations++;
        total_bytes += DATA_SIZE;
        elapsed = (double)(clock() - start) / CLOCKS_PER_SEC;
    } while (iterations < MIN_ITERATIONS || elapsed < MIN_SECONDS);

    double compress_tp = (double)total_bytes / (1024.0 * 1024.0) / elapsed;
    printf("lz4 compress: %d iterations in %.2f seconds\n", iterations, elapsed);
    printf("lz4 compress: throughput = %.2f MB/s\n", compress_tp);

    /* Benchmark decompression */
    iterations = 0;
    total_bytes = 0;
    start = clock();

    do {
        decomp_size = LZ4_decompress_safe(compressed, decompressed, comp_size, DATA_SIZE);
        iterations++;
        total_bytes += DATA_SIZE;
        elapsed = (double)(clock() - start) / CLOCKS_PER_SEC;
    } while (iterations < MIN_ITERATIONS || elapsed < MIN_SECONDS);

    double decompress_tp = (double)total_bytes / (1024.0 * 1024.0) / elapsed;
    printf("lz4 decompress: %d iterations in %.2f seconds\n", iterations, elapsed);
    printf("lz4 decompress: throughput = %.2f MB/s\n", decompress_tp);

    free(input);
    free(compressed);
    free(decompressed);
    return 0;
}
