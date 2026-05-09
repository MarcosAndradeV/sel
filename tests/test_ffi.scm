(define libc (ffi-dlopen "libc.so.6"))

(define strlen (ffi-dlsym libc "strlen"))
(println "Testing strlen via FFI:")
(println (ffi-call strlen 'u64 '(*u8) "Hello, FFI!")) ; Should print 11

(define puts (ffi-dlsym libc "puts"))
(println "Testing puts via FFI:")
(ffi-call puts 'i32 '(*u8) "This string is printed using the C puts function.")

(define libm (ffi-dlopen "libm.so.6"))
(define my_pow (ffi-dlsym libm "pow"))
(println "Testing pow via FFI (2.0 ^ 3.0):")
(println (ffi-call my_pow 'f64 '(f64 f64) 2.0 3.0)) ; Should print 8.0
