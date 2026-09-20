;; tests/test_match.scm - Comprehensive Pattern Matching Tests for sel

;; 1. Literal matching
(define (test-literals x)
  (match x
    (42 "integer")
    (0xFF "hex")
    (0b1010 "bin")
    (3.14 "float")
    ("hello" "string")
    (#t "bool-true")
    (#f "bool-false")
    (#\a "char-a")
    ('foo "symbol-foo")
    (nil "nil-val")
    (_ "other")))

(assert (eq? (test-literals 42) "integer"))
(assert (eq? (test-literals 255) "hex"))
(assert (eq? (test-literals 10) "bin"))
(assert (eq? (test-literals 3.14) "float"))
(assert (eq? (test-literals "hello") "string"))
(assert (eq? (test-literals #t) "bool-true"))
(assert (eq? (test-literals #f) "bool-false"))
(assert (eq? (test-literals #\a) "char-a"))
(assert (eq? (test-literals 'foo) "symbol-foo"))
(assert (eq? (test-literals nil) "nil-val"))
(assert (eq? (test-literals 999) "other"))

;; 2. Wildcard and Variable Binding
(define (test-bindings x)
  (match x
    (0 "zero")
    (n (+ n 10))))

(assert (eq? (test-bindings 0) "zero"))
(assert (eq? (test-bindings 5) 15))

;; 3. Fixed List Patterns
(define (test-list x)
  (match x
    ('() "empty")
    ((1 2 3) "onetwothree")
    ((list a b) (+ a b))
    ((a b c d) (* a (+ b (+ c d))))
    (_ "other")))

(assert (eq? (test-list '()) "empty"))
(assert (eq? (test-list '(1 2 3)) "onetwothree"))
(assert (eq? (test-list '(10 20)) 30))
(assert (eq? (test-list '(2 3 4 5)) 24))
(assert (eq? (test-list '(1)) "other"))

;; 4. Cons Patterns
(define (test-cons x)
  (match x
    ('() 0)
    ((cons h t) (+ h (count t)))))

(assert (eq? (test-cons '()) 0))
(assert (eq? (test-cons '(10)) 10))
(assert (eq? (test-cons '(10 20 30)) 12))

;; Nested Cons
(define (test-nested-cons x)
  (match x
    ((cons (cons a b) rest) (+ a (+ (count b) (count rest))))
    (_ -1)))

(assert (eq? (test-nested-cons '((1 2) 3 4)) 4))
(assert (eq? (test-nested-cons '(1 2 3)) -1))

;; 5. Rest Patterns
(define (test-rest x)
  (match x
    ((1 2 & rest) rest)
    ((list a & rest) (cons a rest))
    (_ nil)))

(assert (eq? (test-rest '(1 2 3 4)) '(3 4)))
(assert (eq? (test-rest '(1 2)) nil))
(assert (eq? (test-rest '(99 100 101)) '(99 100 101)))
(assert (eq? (test-rest '()) nil))

;; 6. Record Patterns
(define (test-record x)
  (match x
    ({type 'point x x_coord y y_coord} (+ x_coord y_coord))
    ({name n age 30} n)
    ({status 'active} "active-user")
    (_ "unknown-record")))

(assert (eq? (test-record {type 'point x 10 y 25}) 35))
(assert (eq? (test-record {name "Alice" age 30 role "admin"}) "Alice"))
(assert (eq? (test-record {status 'active extra 123}) "active-user"))
(assert (eq? (test-record {other 1}) "unknown-record"))

;; Nested Record Pattern
(define (test-nested-record x)
  (match x
    ({user {profile {name n}}} n)
    (_ "fallback")))

(assert (eq? (test-nested-record {user {profile {name "Bob"}}}) "Bob"))
(assert (eq? (test-nested-record {user {profile "incomplete"}}) "fallback"))

;; 7. Guard Clauses (where, when, :where, :when)
(define (test-guards x)
  (match x
    (n (where (< n 0)) "negative")
    (n (when (= n 0)) "zero")
    (n :where (and (> n 0) (< n 10)) "small-positive")
    (n :when (>= n 10) "large-positive")))

(assert (eq? (test-guards -5) "negative"))
(assert (eq? (test-guards 0) "zero"))
(assert (eq? (test-guards 7) "small-positive"))
(assert (eq? (test-guards 42) "large-positive"))

;; Guard failure fallthrough
(define (test-guard-fallthrough x)
  (match x
    (n (where (= n 1)) "one")
    (n (where (= n 2)) "two")
    (n "other-number")))

(assert (eq? (test-guard-fallthrough 1) "one"))
(assert (eq? (test-guard-fallthrough 2) "two"))
(assert (eq? (test-guard-fallthrough 3) "other-number"))

;; 8. Or-patterns
(define (test-or x)
  (match x
    ((or 1 2 3) "1-to-3")
    ((or "yes" "y" #t) "affirmative")
    ((or "no" "n" #f) "negative")
    (_ "other")))

(assert (eq? (test-or 1) "1-to-3"))
(assert (eq? (test-or 2) "1-to-3"))
(assert (eq? (test-or 3) "1-to-3"))
(assert (eq? (test-or "yes") "affirmative"))
(assert (eq? (test-or "y") "affirmative"))
(assert (eq? (test-or #t) "affirmative"))
(assert (eq? (test-or "no") "negative"))
(assert (eq? (test-or #f) "negative"))
(assert (eq? (test-or "maybe") "other"))

;; 9. Error on Exhaustiveness Failure
(define error-caught #f)
(try
  (match 999
    (1 "one")
    (2 "two"))
  (catch err
    (set! error-caught #t)))

(assert (eq? error-caught #t))

;; 10. match-lambda shorthand
(define describe-num
  (match-lambda
    (0 "zero")
    (1 "one")
    (n (+ n 100))))

(assert (eq? (describe-num 0) "zero"))
(assert (eq? (describe-num 1) "one"))
(assert (eq? (describe-num 5) 105))

;; 11. Tail Call Optimization (TCO) with match
(define (countdown-match n acc)
  (match n
    (0 acc)
    (_ (countdown-match (- n 1) (+ acc 1)))))

;; Recurse 20,000 times to verify no stack overflow
(assert (eq? (countdown-match 20000 0) 20000))
