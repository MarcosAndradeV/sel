;; tests/test_for.scm - Tests for List Comprehension (for) in sel

;; 1. Basic single-generator comprehension
(define res1
  (for (x (range 5))
    :do (* x 2)))

(assert (eq? res1 '(0 2 4 6 8)))

;; 2. Comprehension with :when filter
(define res2
  (for (x (range 10))
    :when (= (mod x 2) 0)
    :do (* x x)))

(assert (eq? res2 '(0 4 16 36 64)))

;; 3. Comprehension with :where filter
(define res3
  (for (x (range 6))
    :where (> x 2)
    :do x))

(assert (eq? res3 '(3 4 5)))

;; 4. Multiple generators (cartesian product)
(define res4
  (for (x '(1 2))
       (y '(a b))
    :do (list x y)))

(assert (eq? res4 '((1 a) (1 b) (2 a) (2 b))))

;; 5. Bundled generators syntax: ((x seq1) (y seq2))
(define res5
  (for ((x '(1 2))
        (y '(10 20)))
    :do (+ x y)))

(assert (eq? res5 '(11 21 12 22)))

;; 6. Comprehension with :let local bindings and :when guard
(define res6
  (for (x (range 6))
    :let ((sq (* x x)))
    :when (> sq 5)
    :do sq))

(assert (eq? res6 '(9 16 25)))

;; 7. Pattern matching with automatic filtering of non-matching elements
(define users
  (list {name "Alice" role 'admin}
        {name "Bob" role 'guest}
        {name "Charlie" role 'admin}
        12345))

(define admin-names
  (for ({name n role 'admin} users)
    :do n))

(assert (eq? admin-names '("Alice" "Charlie")))

;; 8. 3-level nested comprehension
(define triples
  (for (x '(1 2))
       (y '(10 20))
       (z '(100 200))
    :do (+ x y z)))

(assert (eq? triples '(111 211 121 221 112 212 122 222)))
