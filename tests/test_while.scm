(define x 0)
(while (< x 10000)
  (set! x (+ x 1)))

(assert (eq? x 10000))

