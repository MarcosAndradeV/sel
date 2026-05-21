;; Test monadic error values

(println "--- Monadic Error Value Tests ---")

;; 1. Basic constructors and predicates
(define v1 (ok 123))
(define v2 (err "something went wrong"))

(assert (ok? v1))
(assert (not (err? v1)))
(assert (err? v2))
(assert (not (ok? v2)))

(assert (= (unwrap v1) 123))
(assert (eq? (error-value v2) "something went wrong"))

(println "Basic ok/err constructors and predicates: PASS")

;; 2. attempt macro wrapping
(define successful-attempt (attempt (+ 1 2)))
(define failing-attempt (attempt (+ 1 "invalid")))

(assert (ok? successful-attempt))
(assert (= (unwrap successful-attempt) 3))

(assert (err? failing-attempt))
(println "Failing attempt error:")
(println (error-value failing-attempt))

(println "attempt macro wrapping: PASS")

;; 3. try-bind monadic chaining
(define divide
  (lambda (x y)
    (if (= y 0)
        (err "Division by zero")
        (ok (/ x y)))))

(define compute-chain
  (lambda (a b c)
    (try-bind (divide a b) val1
      (try-bind (divide val1 c) val2
        (ok (+ val2 10))))))

;; Success chain
(define s-res (compute-chain 20 2 2))
(assert (ok? s-res))
(assert (= (unwrap s-res) 15))

;; Early failure in first step
(define f1-res (compute-chain 20 0 2))
(assert (err? f1-res))
(assert (eq? (error-value f1-res) "Division by zero"))

;; Early failure in second step
(define f2-res (compute-chain 20 2 0))
(assert (err? f2-res))
(assert (eq? (error-value f2-res) "Division by zero"))

(println "try-bind monadic chaining: PASS")
