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
    (throw "odd number of forms to cond")) (cons 'cond (cdr (cdr xs))))))

(defmacro ffi-func (sym ret arg-types)
    (list 'lambda '(&args) `(ffi-call ~sym ~ret ~arg-types args)))

;; BUG: -> and ->> do not work mwith lambda's
;; maybe it's something with macro expansion.
;; or maybe it's the macro expand implementation
(defmacro -> (x & xs)
    (foldl _iter-> x xs))

(defun _iter-> (acc form)
    (if (list? form)
      `(~(car form) ~acc ~@(cdr form))
      (list form acc)))

(defmacro ->> (x & xs)
     (foldl _iter->> x xs))

(defun _iter->> (acc form)
    (if (list? form)
      `(~(car form) ~@(cdr form) ~acc)
      (list form acc)))

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
