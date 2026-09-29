// "standard" style: no semicolons, ASI everywhere
const a = 1
const b = 2
let c = a
+ b
const obj = {
  x: 1
}
const arr = [1, 2]
;[3, 4].forEach(n => arr.push(n))
function f () {
  return
    42
}
function g () {
  return (
    42
  )
}
let i = 0
i
++
i
const s = `t
 x`
const fn = () => {
  return {
    k: 1
  }
}
class K {
  static
  x = 1
  get
  y () { return 2 }
}
console.log(a, b, c, obj.x, arr, f(), g(), i, s, fn().k, K.x, new K().y)
