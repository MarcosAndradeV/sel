(define map (lambda (f xs)
    (if (list? xs)
        (cons (f (car xs)) (map f (cdr xs)))
        nil)
))

(define square (lambda (x)
    (* x x)))

(print (map square (list 2 4 6)))
