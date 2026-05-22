;; 1. Syntax Error: Unclosed list
(define foo (lambda (x) (+ x 1)

;; 2. Unbound/Undefined Variable
(define bar (lambda () unbound-variable-here))

;; 3. Arity Mismatch
(define baz (lambda (a b) (+ a b)))
(baz 1)
