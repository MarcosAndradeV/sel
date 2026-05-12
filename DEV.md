# Idea 2: Signatures and Types
```scm
(deftype number (or 'int 'float))

(signature display (&args (list 'any)) nil)
(signature + (a number b number) number)

;; Type System Implementation
(define _type_registry '())

(defmacro deftype (name type-expr)
  (list 'set! '_type_registry (list 'cons (list 'list (list 'quote name) (list 'quote type-expr)) '_type_registry)))

(define (resolve-type type-name)
  (let ((found (filter (lambda (entry) (eq? (car entry) type-name)) _type_registry)))
    (if (empty? found)
        type-name
        (nth (car found) 1))))

(define (type-match? val type-def)
  (let ((resolved-def (if (symbol? type-def) (resolve-type type-def) type-def)))
    (cond
      (eq? resolved-def 'any) #t
      (symbol? resolved-def) (eq? (type-of val) resolved-def)
      (list? resolved-def)
       (let ((op (car resolved-def)))
         (cond
           (eq? op 'or)
            (foldl (lambda (acc t) (or acc (type-match? val t))) #f (cdr resolved-def))
           (eq? op 'list)
            (and (eq? (type-of val) 'list)
                 (let ((elem-type (nth resolved-def 1)))
                   (foldl (lambda (acc v) (and acc (type-match? v elem-type))) #t val)))
           (eq? op 'quote)
            (eq? (type-of val) (nth resolved-def 1))
           #t (error "Unknown type operator" op)))
      #t (error "Invalid type definition" type-def))))

(define (assert-type val type-def)
  (if (type-match? val type-def)
      val
      (error "TypeMismatch: expected" type-def "but got" val "of type" (type-of val))))

(define (param-name p)
  (if (eq? p '&args)
      'args
      p))

(define (extract-params sig)
  (if (empty? sig)
      '()
      (cons (car sig) (extract-params (cdr (cdr sig))))))

(define (extract-checks sig)
  (if (empty? sig)
      '()
      (let ((pname (param-name (car sig)))
            (ptype (car (cdr sig))))
        (cons (list 'assert-type pname (list 'quote ptype))
              (extract-checks (cdr (cdr sig)))))))

(defmacro signature (func-name sig-args ret-type)
  (let ((params (extract-params sig-args))
        (checks (extract-checks sig-args)))
    (list 'set! func-name
          (list 'let (list (list 'original-fn func-name))
                (list 'lambda params
                      (list 'begin
                            (cons 'begin checks)
                            (list 'let (list (list 'ret (cons 'original-fn (map param-name params))))
                                  (list 'assert-type 'ret (list 'quote ret-type))
                                  'ret)))))))

```
