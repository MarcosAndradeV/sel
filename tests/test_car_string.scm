;; Test car on string
(assert (eq? (car "hello") #\h))
(assert (eq? (car "world") #\w))
(assert (eq? (car "A") #\A))

;; Verify first-class / higher-order car on string
(define my-car car)
(assert (eq? (my-car "test") #\t))
