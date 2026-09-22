;; Test String Utilities

;; string-split
(assert (eq? (string-split "apple,banana,cherry" ",") '("apple" "banana" "cherry")))
(assert (eq? (string-split "foo:bar:baz" #\:) '("foo" "bar" "baz")))
(assert (eq? (string-split "hello" "") '("h" "e" "l" "l" "o")))

;; string-join
(assert (eq? (string-join '("a" "b" "c") "-") "a-b-c"))
(assert (eq? (string-join '("hello" "world") " ") "hello world"))
(assert (eq? (string-join '() ",") ""))

;; string-trim
(assert (eq? (string-trim "   hello world   ") "hello world"))
(assert (eq? (string-trim "\n\t  test \r\n") "test"))
(assert (eq? (string-trim "clean") "clean"))

;; string-replace
(assert (eq? (string-replace "hello world" "world" "sel") "hello sel"))
(assert (eq? (string-replace "banana" "a" "o") "bonono"))
(assert (eq? (string-replace "abc" "z" "x") "abc"))

;; string-upcase & string-downcase
(assert (eq? (string-upcase "hello") "HELLO"))
(assert (eq? (string-upcase "Sel 123!") "SEL 123!"))
(assert (eq? (string-downcase "HELLO WORLD") "hello world"))

;; to-string
(assert (eq? (to-string 42) "42"))
(assert (eq? (to-string 3.14) "3.14"))
(assert (eq? (to-string 'hello) "hello"))
(assert (eq? (to-string #t) "#t"))
(assert (eq? (to-string "already string") "already string"))

;; format
(assert (eq? (format "Hello, {}!" "world") "Hello, world!"))
(assert (eq? (format "{} + {} = {}" 2 3 (+ 2 3)) "2 + 3 = 5"))
(assert (eq? (format "Key: {}, Val: {}" 'name "Marcos") "Key: name, Val: Marcos"))
(assert (eq? (format "no placeholders") "no placeholders"))
