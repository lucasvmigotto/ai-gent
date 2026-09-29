#include <stdio.h>
#include <string.h>

#define SQUARE(x) ((x) * (x))
#define ID (x) /* object-like macro: the space before ( matters */
#define LONG_MACRO(a, b) \
    do {                 \
        (a) += (b);      \
    } while (0)

#ifdef __GNUC__
#  define UNUSED __attribute__((unused))
#else
#  define UNUSED
#endif

typedef struct {
    int rows, cols;
    double data[4][4];
} Matrix;

static int add(int a, int b) { return a + b; }

int (*op)(int, int) = add;

int main(void) {
    Matrix m = {.rows = 2, .cols = 2, .data = {{1, 2}, {3, 4}}};
    int x = 5, y = 3;
    int z = x - -y;          // no fusion
    int w = x+++y;           /* x++ + y */
    int *p = &x;
    int q = *p * *p;         // deref times deref
    const char *msg = "hello "
                      "world";  // adjacent string literal concat
    LONG_MACRO(x, y);
    UNUSED int unused = 0;
    double sum = 0;
    for (int i = 0; i < m.rows; i++)
        for (int j = 0; j < m.cols; j++) sum += m.data[i][j];
    printf("%s %d %d %d %d %.1f %d %d\n", msg, z, w, q, SQUARE(y), sum, op(2, 3), (int)strlen(msg));
    return 0;
}
