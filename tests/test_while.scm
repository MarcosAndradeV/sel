(defmacro while (test &body)
  (list (list 'lambda '(_while_loop_fn_)
              (list '_while_loop_fn_ '_while_loop_fn_))
        (list 'lambda '(_while_loop_fn_)
              (list 'when test
                    (cons 'begin body)
                    (list '_while_loop_fn_ '_while_loop_fn_)))))

(define x 0)
(while (< x 10000)
  (set! x (+ x 1)))
(println x)
