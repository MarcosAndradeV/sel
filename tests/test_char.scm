;; Test Scheme Character Literals

;; Literal characters
(define c1 #\a)
(define c2 #\A)
(define c3 #\()
(define c4 #\ )
(define c5 #\\)

(assert (char? c1))
(assert (char? c2))
(assert (char? c3))
(assert (char? c4))
(assert (char? c5))

;; Special character names
(define s-space #\space)
(define s-newline #\newline)
(define s-tab #\tab)
(define s-return #\return)

(assert (char? s-space))
(assert (char? s-newline))
(assert (char? s-tab))
(assert (char? s-return))

;; Equality tests
(assert (eq? c1 #\a))
(assert (eq? c2 #\A))
(assert (not (eq? c1 c2)))
(assert (eq? s-space #\ ))
(assert (eq? s-space #\space))
(assert (eq? s-newline #\newline))

;; Type checks and predicates
(assert (not (char? 42)))
(assert (not (char? "a")))
(assert (not (char? 'a)))
(assert (eq? (type-of #\a) 'char))

;; Conversions
(assert (eq? (char->integer #\a) 97))
(assert (eq? (char->integer #\A) 65))
(assert (eq? (integer->char 97) #\a))
(assert (eq? (integer->char 65) #\A))
(assert (eq? (char->integer #\space) 32))
(assert (eq? (char->integer #\newline) 10))
