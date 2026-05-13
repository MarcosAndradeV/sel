(define nums '(1 2 3 4 5))

(display "Numbers: ") (display nums) (newline)

(define squared (map (lambda (x) (* x x)) nums))
(display "Squared: ") (display squared) (newline)
(assert (eq? squared '(1 4 9 16 25)))

(define even (filter even? nums))
(display "Even: ") (display even) (newline)
(assert (eq? even '(2 4)))

(when (count nums)
  (display "List has ") (display (count nums)) (display " elements") (newline))

(define rev (reverse nums))
(display "Reversed: ") (display rev) (newline)
(assert (eq? rev '(5 4 3 2 1)))

