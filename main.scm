(define map (lambda (f xs) (if (nil? xs) nil (cons (f (car xs)) (map f (cdr xs))))))

(display (map (lambda (x) (* x x)) '(2 4 6)))
