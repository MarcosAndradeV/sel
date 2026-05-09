;; Sel Core Library

(defmacro when (test &body)
  (list 'if test (cons 'begin body)))

(defmacro unless (test &body)
  (list 'if test 'nil (cons 'begin body)))

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

(defmacro delay (expr)
  (list 'lambda '() expr))

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
