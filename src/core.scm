;; Sel Core Library

; assert :: Bool -> [a] -> ? Error
(define (assert test &args)
    (if (empty? args)
        (when (not test)
            (error "Assertion fail"))
        (when (not test)
            (error "Assertion fail:" args))))

; delay :: Ast -> Ast
(defmacro delay (expr)
  (list 'lambda '() expr))

; map :: (a -> b) -> [a] -> [b]
(define (map f l)
  (if (empty? l)
      '()
      (cons (f (car l)) (map f (cdr l)))))

; filter :: (a -> Bool) -> [a] -> [a]
(define (filter f l)
  (if (empty? l)
      '()
      (if (f (car l))
          (cons (car l) (filter f (cdr l)))
          (filter f (cdr l)))))

; foldl :: (b -> a -> b) -> b -> [a] -> b
(define (foldl f acc l)
  (if (empty? l)
      acc
      (foldl f (f acc (car l)) (cdr l))))

; foldr :: (a -> b -> b) -> b -> [a] -> b
(define (foldr f acc l)
  (if (empty? l)
      acc
      (f (car l) (foldr f acc (cdr l)))))

; reverse :: [a] -> [a]
(define (reverse l)
  (foldl (lambda (acc x) (cons x acc)) '() l))

; repeat :: (->) -> Int
(define (repeat f n)
    (if (<= n 0)
        nil
        (begin (f) (repeat f (- n 1)))))

; force :: (->) -> ? a
(define (force promise)
  (promise))

;; List utilities

; last :: [a] -> ? a
(define (last l)
  (if (empty? (cdr l))
      (car l)
      (last (cdr l))))

; append :: [a] -> [a] -> [a]
(define (append l1 l2)
  (if (empty? l1)
      l2
      (cons (car l1) (append (cdr l1) l2))))

; even? :: Int -> Bool
(define (even? x) (= (mod x 2) 0))

; range-impl :: Int -> Int -> Int -> [Int] -> [Int]
(define (range-impl end start step acc)
    (if (>= start end)
        acc
        (range-impl end (+ step start) step (cons start acc))))

; Just a convinient wrapper
; range :: [Int] -> [Int]
(define (range &args)
    (match args
        ((end)
            (reverse (range-impl end 0 1 '())))
        ((end start)
            (reverse (range-impl end start 1 '())))
        ((end start step)
            (reverse (range-impl end start step '())))
        (_ (error "Arity mismatch in range function\nHint: (range end) (range end begin) (range end begin step)"))
    )
)

;; Monadic Error Values
(define (ok val)
  (list 'ok val))

(define (err msg)
  (list 'err msg))

(define (ok? x)
  (and (list? x) (not (empty? x)) (eq? (car x) 'ok)))

(define (err? x)
  (and (list? x) (not (empty? x)) (eq? (car x) 'err)))

(define (unwrap x)
  (if (ok? x)
      (car (cdr x))
      (error "Called unwrap on err value:" (car (cdr x)))))

(define (error-value x)
  (if (err? x)
      (car (cdr x))
      (error "Called error-value on ok value:" (car (cdr x)))))

(defmacro attempt (expr)
  (list 'try
        (list 'ok expr)
        (list 'catch 'e (list 'err 'e))))

(defmacro try-bind (val var body)
  (list 'let (list (list '_try_bind_res_ val))
        (list 'if (list 'err? '_try_bind_res_)
              '_try_bind_res_
              (list 'let (list (list var (list 'unwrap '_try_bind_res_)))
                    body))))

;; Record helpers
(define (assoc rec k v) (rset rec k v))
(define (dissoc rec k) (rdel rec k))

;; Pattern Matching Helpers
(defmacro match-lambda (&clauses)
  (let ((arg (gensym "arg")))
    (cons 'lambda (cons (list arg)
                        (list (cons 'match (cons arg clauses)))))))

;; Pipeline Operator (thread-last / Elixir & OCaml style for collections)
(defmacro |> (val &forms)
  (if (empty? forms)
      val
      (let ((step (car forms))
            (rest-forms (cdr forms)))
        (let ((next-val (if (list? step)
                            (append step (list val))
                            (list step val))))
          (if (empty? rest-forms)
              next-val
              (cons '|> (cons next-val rest-forms)))))))

;; Thread-first Pipeline (-> / record & object style)
(defmacro -> (val &forms)
  (if (empty? forms)
      val
      (let ((step (car forms))
            (rest-forms (cdr forms)))
        (let ((next-val (if (list? step)
                            (cons (car step) (cons val (cdr step)))
                            (list step val))))
          (if (empty? rest-forms)
              next-val
              (cons '-> (cons next-val rest-forms)))))))

;; Multi-Clause Function Definitions (Elixir / Erlang style)
(defmacro defn (head &clauses)
  (if (list? head)
      (let ((args (cdr head)))
        (if (= (count args) 1)
            (let ((arg (car args)))
              (list 'define head
                    (cons 'match (cons arg clauses))))
            (list 'define head
                  (cons 'match (cons (cons 'list args) clauses)))))
      (let ((arg (gensym "arg")))
        (list 'define (list head arg)
              (cons 'match (cons arg clauses))))))

;; Railway-Oriented with construct (Elixir style)
(define (with-collect-until-marker marker items acc)
  (if (empty? items)
      (list (reverse acc) '())
      (if (eq? (car items) marker)
          (list (reverse acc) (cdr items))
          (with-collect-until-marker marker (cdr items) (cons (car items) acc)))))

(define (with-build-chain clauses body else-handler-sym)
  (if (empty? clauses)
      (if (empty? body)
          'nil
          (if (= (count body) 1)
              (car body)
              (cons 'begin body)))
      (let ((c (car clauses))
            (nested (with-build-chain (cdr clauses) body else-handler-sym))
            (fail-sym (gensym "fail")))
        (let ((fail-clause (list fail-sym (list else-handler-sym fail-sym))))
          (if (= (count c) 2)
              (let ((pat (car c))
                    (expr (nth c 1)))
                (list 'match expr
                      (list pat nested)
                      fail-clause))
              (if (= (count c) 4)
                  (let ((pat (car c))
                        (guard-kw (nth c 1))
                        (guard-expr (nth c 2))
                        (expr (nth c 3)))
                    (list 'match expr
                          (list pat guard-kw guard-expr nested)
                          fail-clause))
                  (error "Invalid clause format in with" c)))))))

(defmacro with (&args)
  (let ((split-do (with-collect-until-marker ':do args '())))
    (let ((before-do (car split-do))
          (after-do (nth split-do 1)))
      (let ((parsed-sections
             (if (empty? after-do)
                 (if (and (not (empty? before-do)) (list? (car before-do)))
                     (list (car before-do) (cdr before-do) '())
                     (error "with requires :do or clauses list"))
                 (let ((clauses (if (and (= (count before-do) 1)
                                         (list? (car before-do))
                                         (not (empty? (car before-do)))
                                         (list? (car (car before-do))))
                                    (car before-do)
                                    before-do)))
                   (let ((split-else (with-collect-until-marker ':else after-do '())))
                     (list clauses (car split-else) (nth split-else 1)))))))
        (let ((clauses (car parsed-sections))
              (body (nth parsed-sections 1))
              (else-clauses (nth parsed-sections 2)))
          (let ((err-sym (gensym "err"))
                (else-handler-sym (gensym "else_fn")))
            (let ((else-fn-def (if (empty? else-clauses)
                                   (list 'lambda (list err-sym) err-sym)
                                   (cons 'lambda (cons (list err-sym)
                                                       (list (cons 'match (cons err-sym else-clauses))))))))
              (list 'let (list (list else-handler-sym else-fn-def))
                    (with-build-chain clauses body else-handler-sym)))))))))

;; List Comprehension (Elixir / Python style)
(define (for-parse-sections args)
  (if (and (not (empty? args))
           (list? (car args))
           (not (empty? (car args)))
           (list? (car (car args))))
      ;; First argument is a list of generators: ((x seq1) (y seq2))
      (let ((gens (car args))
            (rest (cdr args)))
        (for-parse-rest rest gens '() '()))
      ;; Generators are unbundled before :let/:when/:where/:do
      (for-parse-unbundled args '() '() '())))

(define (for-parse-rest args gens let-bindings when-cond)
  (if (empty? args)
      (list gens let-bindings when-cond '())
      (let ((head (car args)))
        (if (eq? head ':do)
            (list gens let-bindings when-cond (cdr args))
            (if (eq? head ':let)
                (for-parse-rest (cdr (cdr args)) gens (car (cdr args)) when-cond)
                (if (or (eq? head ':when) (eq? head ':where))
                    (for-parse-rest (cdr (cdr args)) gens let-bindings (car (cdr args)))
                    (error "Unexpected token in for" head)))))))

(define (for-parse-unbundled args gens let-bindings when-cond)
  (if (empty? args)
      (list (reverse gens) let-bindings when-cond '())
      (let ((head (car args)))
        (if (eq? head ':do)
            (list (reverse gens) let-bindings when-cond (cdr args))
            (if (eq? head ':let)
                (for-parse-unbundled (cdr (cdr args)) gens (car (cdr args)) when-cond)
                (if (or (eq? head ':when) (eq? head ':where))
                    (for-parse-unbundled (cdr (cdr args)) gens let-bindings (car (cdr args)))
                    (for-parse-unbundled (cdr args) (cons head gens) let-bindings when-cond)))))))

(define (for-build-loop gens body-expr let-bindings when-cond is-innermost)
  (if (empty? gens)
      body-expr
      (let ((g (car gens))
            (rest-gens (cdr gens)))
        (let ((pat (car g))
              (seq (car (cdr g)))
              (item-sym (gensym "item"))
              (acc-sym (gensym "acc")))
          (if (empty? rest-gens)
              ;; Innermost generator
              (let ((if-action
                     (if (empty? when-cond)
                         (list 'cons body-expr acc-sym)
                         (list 'if when-cond
                               (list 'cons body-expr acc-sym)
                               acc-sym))))
                (let ((inner-action
                       (if (empty? let-bindings)
                           if-action
                           (list 'let let-bindings if-action))))
                  (list 'foldr
                        (list 'lambda (list item-sym acc-sym)
                              (list 'match item-sym
                                    (list pat inner-action)
                                    (list '_ acc-sym)))
                        ''()
                        seq)))
              ;; Outer generator
              (let ((inner-loop (for-build-loop rest-gens body-expr let-bindings when-cond #f)))
                (list 'foldr
                      (list 'lambda (list item-sym acc-sym)
                            (list 'match item-sym
                                  (list pat (list 'append inner-loop acc-sym))
                                  (list '_ acc-sym)))
                      ''()
                      seq)))))))

(defmacro for (&args)
  (let ((parsed (for-parse-sections args)))
    (let ((gens (car parsed))
          (let-bindings (nth parsed 1))
          (when-cond (nth parsed 2))
          (body (nth parsed 3)))
      (if (empty? gens)
          (error "for requires at least one generator")
          (let ((core-body (if (empty? body)
                               'nil
                               (if (= (count body) 1)
                                   (car body)
                                   (cons 'begin body)))))
            (for-build-loop gens core-body let-bindings when-cond #t))))))
