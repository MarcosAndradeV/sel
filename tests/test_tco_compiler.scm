(defun is-even? (n)
  (if (= n 0)
      #t
      (is-odd? (- n 1))))

(defun is-odd? (n)
  (if (= n 0)
      #f
      (is-even? (- n 1))))

;; Mutual recursion test (10,000 iterations to verify mutually recursive TCO)
(assert (eq? (is-even? 10000) #t))
(assert (eq? (is-odd? 10000) #f))

;; TCO inside let and begin
(defun sum-let-begin (n acc)
  (if (= n 0)
      (begin
        acc)
      (let ((next-n (- n 1))
            (next-acc (+ n acc)))
        (sum-let-begin next-n next-acc))))

(assert (eq? (sum-let-begin 10000 0) 50005000))
