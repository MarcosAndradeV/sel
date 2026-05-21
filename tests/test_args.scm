(define expected '("tests/test_args.scm"))
(assert (eq? (system 'args) expected))
