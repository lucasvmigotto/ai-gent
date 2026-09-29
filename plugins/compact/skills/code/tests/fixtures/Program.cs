using System;
using System.Collections.Generic;
using System.Linq;

#region Models
namespace Demo
{
    public record Person(string Name, int Age);

    public static class Program
    {
        /// <summary>XML doc comment.</summary>
        private const string Verbatim = @"C:\path\
  second line";

        private static readonly string Raw = """
            raw string literal
              keeps its indentation
            """;

        public static int Twice(this int x) => x * 2;

        public static async System.Threading.Tasks.Task Main()
        {
            var people = new List<Person> { new("Ann", 31), new("Bob", 17) };
#if DEBUG
            Console.WriteLine("debug build");
#endif
            var adults = people.Where(p => p.Age >= 18)
                               .Select(p => p.Name)
                               .ToList();
            int a = 5, b = -3;
            int c = a - -b;
            string d = a switch { > 3 => "big", _ => "small" };
            object o = 42;
            if (o is int n && n > 40) Console.WriteLine($"n={n,5} {a.Twice()}");
            await System.Threading.Tasks.Task.Yield();
            Dictionary<string, List<int>> map = new() { ["k"] = new() { 1, 2 } };
            Console.WriteLine($"{string.Join(",", adults)} {c} {d} {map["k"].Count}");
            Console.WriteLine(Verbatim + Raw);
        }
    }
}
#endregion
