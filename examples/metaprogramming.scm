;; Metaprogramming and Macro Wizardry in sel Lisp
;; This example showcases the power of the compile-time macro expansion system.

(println "=== Sel Metaprogramming & Macros Demo ===")

;; Let's define a 'dotimes' macro to easily repeat a block N times, binding the index variable.
;; Because sel performs strict parameter validation at parse-time, we construct the expanded AST list.
(defmacro dotimes (binding &body)
  (define var (car binding))
  (define limit (car (cdr binding)))
  (list (list 'lambda '()
              (list 'define 'loop
                    (list 'lambda (list var)
                          (list 'when (list '< var limit)
                                (cons 'begin body)
                                (list 'loop (list '+ var 1)))))
              (list 'loop 0))))

;; Test the dotimes macro
(println "Printing 0 to 4 using `dotimes` macro:")
(dotimes (i 5)
  (println "  Iteration i =" i))

;; An elegant pipeline example using the thread-suffix `->` operator
;; Note: in sel, (-> x (f a)) appends x to the end of the argument list: (f a x)
(define double-num (lambda (x) (* x 2)))
(define add-ten (lambda (x) (+ x 10)))
(define display-result (lambda (label x) (println label ":" x)))

(println "\nExecuting pipeline with `->` operator:")
;; This evaluates: (display-result "Final Pipeline Result" (add-ten (double-num 5)))
(-> 5
    (double-num)
    (add-ten)
    (display-result "Final Pipeline Result"))
