#include <algorithm>
#include <iostream>
#include <map>
#include <string>
#include <vector>

namespace g {
template <typename T>
using Adj = std::vector<std::vector<T>>;

template <typename K, typename V>
class Registry {
   public:
    void put(const K& k, V v) { m_[k] = std::move(v); }
    const V* get(const K& k) const {
        auto it = m_.find(k);
        return it == m_.end() ? nullptr : &it->second;
    }

   private:
    std::map<K, V> m_;
};
}  // namespace g

struct Point {
    int x, y;
    Point operator+(const Point& o) const { return {x + o.x, y + o.y}; }
    bool operator<(const Point& o) const { return x < o.x || (x == o.x && y < o.y); }
};

int main() {
    g::Adj<int> adj(3);
    adj[0].push_back(1);
    adj[1].push_back(2);
    auto raw = R"(raw "string"
   with  spaces)";
    g::Registry<std::string, int> reg;
    reg.put("a", 1);
    int b = 2;
    auto f = [&b](int v) -> int { return v * b; };
    std::vector<Point> pts{{3, 1}, {1, 2}, {1, 1}};
    std::sort(pts.begin(), pts.end());
    Point s = pts[0] + pts[1];
    int t = b > 1 ? b : ::abs(-b);
    std::cout << raw << "|" << *reg.get("a") << " " << f(21) << " " << s.x << "," << s.y
              << " " << adj[1][0] << " " << t << (b >> 1) << std::endl;
    return 0;
}
