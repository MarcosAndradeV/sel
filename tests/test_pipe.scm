;; tests/test_pipe.scm - Tests for the |> pipeline operator in sel

;; 1. Bare values with no pipeline steps
(assert (= (|> 42) 42))

;; 2. Pipeline with bare functions
(define (square x) (* x x))
(define (add-ten x) (+ x 10))

(assert (= (|> 5 square) 25))
(assert (= (|> 5 square add-ten) 35))

;; 3. Pipeline with function calls (thread-last)
(define (sub a b) (- a b))
;; (|> 10 (sub 30)) -> (sub 30 10) -> 20
(assert (= (|> 10 (sub 30)) 20))

;; 4. Pipeline with collections: map, filter, reverse
(define numbers '(1 2 3 4 5))
(define result
  (|> numbers
      (map \(x) (* x 2))
      (filter \(x) (> x 4))
      reverse))

(assert (eq? result '(10 8 6)))

;; 5. Pipeline with string and arithmetic operations
(assert (= (|> 5 (+ 5) (* 2)) 20))

;; 6. Thread-first (->) tests for records and data objects
(define p {x 10 y 20})
(define p2 (-> p (assoc 'x 100) (assoc 'z 300)))
(assert (= (rget p2 'x) 100))
(assert (= (rget p2 'y) 20))
(assert (= (rget p2 'z) 300))

;; (-> 30 (sub 10)) -> (sub 30 10) -> 20
(assert (= (-> 30 (sub 10)) 20))
