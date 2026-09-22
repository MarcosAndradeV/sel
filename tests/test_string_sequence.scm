;; tests/test_string_sequence.scm - String as List-of-Chars Sequence Semantics

;; 1. car and cdr
(assert (eq? (car "hello") #\h))
(assert (eq? (cdr "hello") "ello"))
(assert (eq? (car (cdr "hello")) #\e))
(assert (eq? (cdr (cdr "hello")) "llo"))
(assert (eq? (cdr "h") '()))
(assert (eq? (car "") '()))
(assert (eq? (cdr "") '()))

;; 2. cons
(assert (eq? (cons #\h "ello") "hello"))
(assert (eq? (cons #\a "") "a"))
(assert (eq? (cons #\x (cdr "xyz")) "xyz"))

;; 3. drop and nth
(assert (eq? (drop 0 "hello") "hello"))
(assert (eq? (drop 2 "hello") "llo"))
(assert (eq? (drop 5 "hello") '()))
(assert (eq? (drop 10 "hello") '()))

(assert (eq? (nth "hello" 0) #\h))
(assert (eq? (nth "hello" 1) #\e))
(assert (eq? (nth "hello" 4) #\o))
(assert (eq? (nth "hello" 5) '()))
(assert (eq? (nth (cdr "world") 2) #\l))

;; 4. count and empty?
(assert (= (count "hello") 5))
(assert (= (count "") 0))
(assert (empty? ""))
(assert (not (empty? "a")))
(assert (not (empty? "hello")))

;; 5. Predicates: list? and string?
(assert (list? "hello"))
(assert (list? ""))
(assert (string? "hello"))
(assert (string? ""))
(assert (string? '(#\h #\i)))
(assert (not (string? '(1 2 3))))

;; 6. Equality between String and List of Chars
(assert (eq? "hi" '(#\h #\i)))
(assert (eq? '(#\h #\i) "hi"))
(assert (eq? "" '()))
(assert (eq? '() ""))
(assert (eq? (cdr "abc") "bc"))

;; 7. Multi-byte UTF-8 handling
(assert (= (count "こんにちは") 5))
(assert (eq? (car "こんにちは") #\こ))
(assert (eq? (cdr "こんにちは") "んにちは"))
(assert (eq? (nth "こんにちは" 2) #\に))
(assert (eq? (drop 3 "こんにちは") "ちは"))
(assert (eq? (cons #\こ "んにちは") "こんにちは"))

;; 8. Pattern Matching on Strings: Cons Pattern
(define (match-str-cons s)
  (match s
    ('() 'empty)
    ((cons h t) (list h t))))

(assert (eq? (match-str-cons "") 'empty))
(assert (eq? (match-str-cons "a") '(#\a ())))
(assert (eq? (match-str-cons "hello") '(#\h "ello")))

;; 9. Pattern Matching on Strings: Fixed List Pattern
(define (match-str-fixed s)
  (match s
    ('() "empty")
    ((a b c) (list a b c))
    (_ "other")))

(assert (eq? (match-str-fixed "") "empty"))
(assert (eq? (match-str-fixed "abc") '(#\a #\b #\c)))
(assert (eq? (match-str-fixed "ab") "other"))
(assert (eq? (match-str-fixed "abcd") "other"))

;; 10. Multi-clause defn with string pattern matching
(defn (string-head-tail s)
  ("" :do 'empty)
  ((cons h t) :do (list h t)))

(assert (eq? (string-head-tail "") 'empty))
(assert (eq? (string-head-tail "sel") '(#\s "el")))

