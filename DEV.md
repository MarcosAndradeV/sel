# Idea 1: Conditional libc IO

```scm
(when (eq? (os/getenv "SEL_USE_LIBC") "true")
    (define libc (ffi-dlopen "libc.so.6"))
    (define puts-sym (ffi-dlsym libc "puts"))
    (define puts (ffi-func puts-sym 'i32 '(*u8)))
)

(puts "Hello, world")
```

# Idea 2: Signatures and Types
```scm
(deftype number (or 'int 'float))

(signature display (&args (list 'any)) nil)
(signature + (a number b number) number)
```
