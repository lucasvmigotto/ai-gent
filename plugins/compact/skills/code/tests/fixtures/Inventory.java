package demo;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.stream.Collectors;

/**
 * Inventory demo exercising tricky lexical constructs.
 *
 * <p>Javadoc with   odd   spacing must survive verbatim.
 */
public class Inventory {
    // a line comment right before a field: the newline after it is mandatory
    private static final String BANNER = """
        Inventory report
          indented line inside a text block
        """;

    record Item(String name, int qty, double price) {}

    sealed interface Shape permits Circle, Square {}

    record Circle(double r) implements Shape {}

    record Square(double s) implements Shape {}

    private final List<Item> items = new ArrayList<>();
    private final Map<String, List<Map<String, Integer>>> nested = Map.of();

    @SuppressWarnings("unchecked")
    public <T extends Comparable<T>> T max(T a, T b) {
        return a.compareTo(b) >= 0 ? a : b;
    }

    static double area(Shape s) {
        return switch (s) {
            case Circle c -> Math.PI * c.r() * c.r();
            case Square q -> q.s() * q.s();
        };
    }

    public static void main(String[] args) {
        Inventory inv = new Inventory();
        inv.items.add(new Item("apple", 3, 0.5));
        inv.items.add(new Item("pear", 7, 0.75));
        int i = 5, j = 2;
        int k = i - -j;       // minus minus must not fuse
        int m = i++ + ++j;    // plus plus plus plus
        char c = ' ';         // a space char literal
        String s = "a  b\tc";  /* block comment */ String t = "x" + 'y';
        Function<Integer, Integer> twice = x -> x * 2;
        outer:
        for (int a = 0; a < 3; a++) {
            for (int b = 0; b < 3; b++) {
                if (a * b > 1) break outer;
            }
        }
        String names = inv.items.stream()
                .filter(it -> it.qty() > 1)
                .map(Item::name)
                .collect(Collectors.joining(", "));
        int[] arr = {1, 2, 3};
        Runnable r = new Runnable() {
            @Override
            public void run() {
                System.out.println("anon " + arr.length);
            }
        };
        r.run();
        System.out.print(BANNER);
        System.out.println(names + " " + k + " " + m + " [" + c + "] " + s + t + twice.apply(21));
        System.out.println(inv.max("abc", "abd") + " " + area(new Circle(1.0)) + " " + area(new Square(2)));
        System.out.println(i >> 1 >>> 1 << 2);
    }
}
