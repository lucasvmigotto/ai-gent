package demo

import kotlin.math.PI

/** KDoc comment. */
sealed class Shape {
    abstract val area: Double

    data class Circle(val r: Double) : Shape() {
        override val area: Double
            get() = PI * r * r
    }

    data class Rect(val w: Double, val h: Double) : Shape() {
        override val area get() = w * h
    }
}

object Registry {
    private val items = mutableListOf<Shape>()

    fun add(s: Shape): Registry {
        items += s
        return this
    }

    val total: Double
        get() = items.sumOf { it.area }
}

fun describe(x: Any?): String =
    when (x) {
        null -> "null"
        is Int -> if (x > 0) "positive" else "non-positive"
        is String -> "string of ${x.length}"
        else -> "other"
    }

inline fun <reified T> List<*>.firstOf(): T? = firstOrNull { it is T } as? T

fun main() {
    val raw = """
        |multi-line
        |   raw string with ${1 + 1} template
    """.trimMargin()
    val names = listOf("b", "a", "c")
        .sorted()
        .map { it.uppercase() }
        ?.joinToString(separator = ",")
        ?: "none"
    var n = 0
    for (i in 1..10) {
        if (i % 2 == 0) continue
        n += i
    }
    val m = -n
    val neg = n - -3
    val nullable: String? = null
    val len = nullable?.length ?: -1
    Registry.add(Shape.Circle(1.0)).add(Shape.Rect(2.0, 3.0))
    val res = run {
        val a = 2
        a * 21
    }
    listOf(1, 2, 3).forEach {
        if (it == 2) return@forEach
        print(it)
    }
    println()
    println(raw)
    println("$names $n $m $neg $len $res ${describe(5)} ${describe("hey")}")
    println(listOf(1, "x", 2.0).firstOf<String>())
    println("%.2f".format(Registry.total))
}
