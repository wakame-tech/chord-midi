

```
# 繰り返し使うリズム
$返し   = "o.oo.oo."
$B刻み = "o..o..oo"

# Aメロ
$A1 = pat("o.oo..oo", pow(A@6 | A@6 | D@5)) pat($返し, pow(E@5))
$A2 = pat($返し, pow(D@5 | E@5)) pat("o.oo.ooo", pow(A@6)) pat("xxo..oo.", pow(A@6))

# Bメロ
$B1 = pat($B刻み, pow(D@5 | D@5 | Db@5 | F#@6 | B@5 | Db@5 | F#@6)) pat("xxo.o.o.", oct(G#@6 G#@6 A@6 C#@6))
$B2 = pat($B刻み, pow(D@5 | D@5 | Db@5 | F#@6 | B@5 | D@5))
$B3 = pat("o.o.oooo", pow(E@5)) pat("xxo.o.oo", pow(E@5))

# A
$A1 x3
$A2
# B
$B1
$B2
$B3
```

