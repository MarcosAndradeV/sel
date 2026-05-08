(define nums '(1 2 3 4 5))

(display "Numbers: ") (display nums) (newline)

(define squared (map (lambda (x) (* x x)) nums))
(display "Squared: ") (display squared) (newline)

(define even (filter even? nums))
(display "Even: ") (display even) (newline)

(when (count nums)
  (display "List has ") (display (count nums)) (display " elements") (newline))

(define rev (reverse nums))
(display "Reversed: ") (display rev) (newline)
