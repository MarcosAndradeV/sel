(define lib (ffi-dlopen "./libffi_test_structs.so"))

;; 1. Test 16-bit signed short addition ('i16)
(define add-shorts (ffi-func (ffi-dlsym lib "add_shorts") 'i16 '(i16 i16)))

(println "Testing add_shorts (i16):")
(assert (eq? (add-shorts 100 200) 300))
(assert (eq? (add-shorts -50 30) -20))
(assert (eq? (add-shorts 32760 5) 32765))

;; 2. Test 8-bit unsigned character operations ('u8 and 'uchar)
(define next-char-u8 (ffi-func (ffi-dlsym lib "next_char") 'u8 '(u8)))
(define next-char-uchar (ffi-func (ffi-dlsym lib "next_char") 'uchar '(uchar)))

(println "Testing next_char (u8 / uchar):")
(assert (eq? (next-char-u8 65) 66))
(assert (eq? (next-char-uchar 254) 255))
;; 255 wraps to 0 due to overflow in C unsigned char
(assert (eq? (next-char-uchar 255) 0))

;; 3. Test 8-bit signed character operations ('i8, 'ichar, and 'char)
(define invert-i8 (ffi-func (ffi-dlsym lib "invert_char_sign") 'i8 '(i8)))
(define invert-ichar (ffi-func (ffi-dlsym lib "invert_char_sign") 'ichar '(ichar)))
(define invert-char (ffi-func (ffi-dlsym lib "invert_char_sign") 'char '(char)))

(println "Testing invert_char_sign (i8 / ichar / char):")
(assert (eq? (invert-i8 42) -42))
(assert (eq? (invert-ichar -15) 15))
(assert (eq? (invert-char 127) -127))

(println "All FFI types and aliases integration tests passed successfully!")
