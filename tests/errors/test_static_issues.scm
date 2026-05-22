;; 1. Unbound variable
(define foo (lambda (x) (+ x y)))

;; 2. Arity mismatch
(define bar (lambda (a b) (+ a b)))
(bar 1)
