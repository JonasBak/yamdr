# Yet another markdown renderer

```{"t": "Graph"}
digraph D {

  A;
  B;
  C;
  D;
  E;

  A -- B;
  A -- C;
  A -- D;
  B -- E;
  C -- E;
  D -- E;

}
```

```{"t": "ScriptGlobals"}
fn fib(n) {
    if n < 2 {
        n
    } else {
        fib(n-1) + fib(n-2)
    }
}
```

```{"t": "Script"}
let x = 4 + 5;
x;

debug(x);
debug(fib(5));
```

`_x_`

```{"t": "Data"}
name: test
data:
- fieldA: a
  fieldB: b
- fieldA: a
  fieldB: b
  fieldC: c
```
