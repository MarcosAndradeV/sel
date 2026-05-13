(define expected '("testsfsd/test_args.scm"))
(println (eq? (os/args) expected))

(assert (eq? (os/args) expected))

; NOTE: os/orig-args works but if you run with cargo r it outputs the target/debug/sel path
; (assert (eq? (os/orig-args) '("sel" "tests/test_args.scm")))
