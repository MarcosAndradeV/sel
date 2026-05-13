(define x '(2 3))
(define y `(1 ~@x 4))
(assert (eq? y '(1 2 3 4)))

