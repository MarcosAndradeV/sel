(assert (eq? (if #t 1 2) 1))
(assert (eq? (if #f 1 2) 2))
(assert (eq? (begin (define x 10) (+ x 5)) 15))
(assert (eq? (let ((y 20)) (+ y 10)) 30))
(assert (eq? (and #t #t #t) #t))
(assert (eq? (and #t #f #t) #f))
(assert (eq? (or #f #f #t) #t))
(assert (eq? (or #f #f #f) #f))

