(define libc (ffi-dlopen "libc.so.6"))

;; using raw ffi-call
(define puts (ffi-dlsym libc "puts"))
(println "Testing puts via FFI:")
(ffi-call puts 'i32 '(*u8) '("This string is printed using the C puts function."))

;; using ffi-func
(define strlen-sym (ffi-dlsym libc "strlen"))
(define strlen (ffi-func strlen-sym 'u64 '(*u8)))

(println "Testing strlen via FFI:")
(println (strlen "Hello, FFI!")) ; Should print 11

;; math

(define libm (ffi-dlopen "libm.so.6"))
(define my_pow (ffi-dlsym libm "pow"))
(println "Testing pow via FFI (2.0 ^ 3.0):")
(println (ffi-call my_pow 'f64 '(f64 f64) '(2.0 3.0))) ; Should print 8.0
