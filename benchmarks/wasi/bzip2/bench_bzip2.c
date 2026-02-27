/*
 * bzip2 compression benchmark for WASI
 * Compresses a synthetic data buffer repeatedly and reports throughput.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "bzlib.h"

#define DATA_SIZE (256 * 1024)  /* 256 KB input buffer */
#define MIN_ITERATIONS 20
#define MIN_SECONDS 10.0

/* Generate pseudo-random but compressible data (English-like text patterns) */
static void generate_data(unsigned char *buf, int size) {
    const char *words[] = {
        "the ", "quick ", "brown ", "fox ", "jumps ", "over ", "lazy ", "dog ",
        "hello ", "world ", "benchmark ", "compression ", "data ", "test ",
        "performance ", "wasm ", "runtime ", "function ", "value ", "return ",
    };
    int nwords = 20;
    unsigned int state = 0xDEADBEEF;
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
    /* bzip2 output can be slightly larger than input in worst case */
    unsigned int out_size = DATA_SIZE + DATA_SIZE / 100 + 600;
    char *output = malloc(out_size);

    if (!input || !output) {
        fprintf(stderr, "Failed to allocate memory\n");
        return 1;
    }

    generate_data(input, DATA_SIZE);

    /* Warm up */
    unsigned int dest_len = out_size;
    int rc = BZ2_bzBuffToBuffCompress(output, &dest_len, (char *)input, DATA_SIZE, 9, 0, 30);
    if (rc != BZ_OK) {
        fprintf(stderr, "bzip2 compress failed: %d\n", rc);
        return 1;
    }

    printf("bzip2 benchmark: %d KB input -> %u KB compressed (%.1fx)\n",
           DATA_SIZE / 1024, dest_len / 1024, (double)DATA_SIZE / dest_len);

    /* Benchmark loop */
    int iterations = 0;
    long long total_bytes = 0;
    clock_t start = clock();
    double elapsed;

    do {
        dest_len = out_size;
        rc = BZ2_bzBuffToBuffCompress(output, &dest_len, (char *)input, DATA_SIZE, 9, 0, 30);
        if (rc != BZ_OK) {
            fprintf(stderr, "bzip2 compress failed: %d\n", rc);
            return 1;
        }
        iterations++;
        total_bytes += DATA_SIZE;
        elapsed = (double)(clock() - start) / CLOCKS_PER_SEC;
    } while (iterations < MIN_ITERATIONS || elapsed < MIN_SECONDS);

    double throughput = (double)total_bytes / (1024.0 * 1024.0) / elapsed;
    printf("bzip2: %d iterations in %.2f seconds\n", iterations, elapsed);
    printf("bzip2: throughput = %.2f MB/s\n", throughput);

    free(input);
    free(output);
    return 0;
}
