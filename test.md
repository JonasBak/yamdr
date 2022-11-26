# Yet another markdown renderer

```{"t": "Test"}
abc
```

```{"t": "Graph"}
abc
```

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

```{"t": "aa"}
abc
```

```{"t": "Script"}
let x = 4 + 5;
x;

fn fib(n) {
    if n < 2 {
        n
    } else {
        fib(n-1) + fib(n-2)
    }
}

debug(fib(5));
```
