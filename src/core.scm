;; Sel Core Library

(defmacro when (test &body)
  (list 'if test (cons 'begin body)))

(defmacro unless (test &body)
  (list 'if test 'nil (cons 'begin body)))

(defmacro while (test &body)
  (list (list 'lambda '(_while_loop_fn_)
              (list '_while_loop_fn_ '_while_loop_fn_))
        (list 'lambda '(_while_loop_fn_)
              (list 'when test
                    (cons 'begin body)
                    (list '_while_loop_fn_ '_while_loop_fn_)))))

(defmacro until (test &body)
  (list (list 'lambda '(_until_loop_fn_)
              (list '_until_loop_fn_ '_until_loop_fn_))
        (list 'lambda '(_until_loop_fn_)
              (list 'unless test
                    (cons 'begin body)
                    (list '_until_loop_fn_ '_until_loop_fn_)))))

(defmacro defun (name args &body)
    (list 'define `(~name ~@args) (cons 'begin body)))

(defmacro cond (& xs)
    (if (> (count xs) 0)
    (list 'if (car xs)
    (if (> (count xs) 1)
    (nth xs 1)
    (error "odd number of forms to cond")) (cons 'cond (cdr (cdr xs))))))

(defmacro ffi-func (sym ret arg-types)
    (list 'lambda '(&args) `(ffi-call ~sym ~ret ~arg-types args)))

(defun assert (test &args)
    (if (empty? args)
        (when (not test)
            (error "Assertion fail"))
        (when (not test)
            (error "Assertion fail:" args))))

(defmacro delay (expr)
  (list 'lambda '() expr))

(define (map f l)
  (if (empty? l)
      '()
      (cons (f (car l)) (map f (cdr l)))))

(define (filter f l)
  (if (empty? l)
      '()
      (if (f (car l))
          (cons (car l) (filter f (cdr l)))
          (filter f (cdr l)))))

(define (foldl f acc l)
  (if (empty? l)
      acc
      (foldl f (f acc (car l)) (cdr l))))

(define (foldr f acc l)
  (if (empty? l)
      acc
      (f (car l) (foldr f acc (cdr l)))))

(define (reverse l)
  (foldl (lambda (acc x) (cons x acc)) '() l))

(define (repeat f n)
    (if (<= n 0)
        nil
        (begin (f) (repeat f (- n 1)))))

(define (force promise)
  (promise))

;; List utilities
(define (last l)
  (if (empty? (cdr l))
      (car l)
      (last (cdr l))))

(define (append l1 l2)
  (if (empty? l1)
      l2
      (cons (car l1) (append (cdr l1) l2))))

(define (even? x) (= (mod x 2) 0))

(define (range-impl end begin step acc)
    (if (>= begin end)
        acc
        (range-impl end (+ step begin) step (cons begin acc))))

; Just a convinient wrapper
(define (range &args)
    (cond
        (= (count args) 1)
            (reverse (range-impl (car args) 0 1 '()))
        (= (count args) 2)
            (reverse (range-impl (car args) (nth args 1) 1 '()))
        (= (count args) 3)
            (reverse (range-impl (car args) (nth args 1) (nth args 2) '()))
        #t (error "Arity mismatch in range function\nHint: (range end) (range end begin) (range end begin step)")
    )
)
