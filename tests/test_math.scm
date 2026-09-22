;; Test Math Library & Constants

;; Constants
(assert (> pi 3.14))
(assert (< pi 3.15))
(assert (> tau 6.28))
(assert (< tau 6.29))
(assert (> e 2.71))
(assert (< e 2.72))

;; abs
(assert (= (abs -10) 10))
(assert (= (abs 10) 10))
(assert (= (abs 0) 0))
(assert (= (abs -3.14) 3.14))
(assert (= (abs 3.14) 3.14))

;; min & max
(assert (= (min 5 2 9 1 7) 1))
(assert (= (min 42) 42))
(assert (= (min 3 1.5 8) 1.5))
(assert (= (max 5 2 9 1 7) 9))
(assert (= (max -5 -20 -3) -3))
(assert (= (max 4.2 4 1) 4.2))

;; sqrt
(assert (= (sqrt 0) 0.0))
(assert (= (sqrt 4) 2.0))
(assert (= (sqrt 16) 4.0))
(assert (= (sqrt 2.25) 1.5))

;; pow
(assert (= (pow 2 3) 8))
(assert (= (pow 10 0) 1))
(assert (= (pow 5 2) 25))
(assert (= (pow 4 0.5) 2.0))

;; floor, ceil, round
(assert (= (floor 3.8) 3))
(assert (= (floor 3.1) 3))
(assert (= (floor 5) 5))
(assert (= (ceil 3.1) 4))
(assert (= (ceil 3.9) 4))
(assert (= (ceil 5) 5))
(assert (= (round 3.4) 3))
(assert (= (round 3.6) 4))

;; Trigonometry
(assert (= (sin 0) 0.0))
(assert (= (cos 0) 1.0))
(assert (< (abs (tan 0)) 0.0001))

;; Bitwise operations
(assert (= (bit-and 12 10) 8))          ; 1100 & 1010 = 1000
(assert (= (bit-or 12 10) 14))          ; 1100 | 1010 = 1110
(assert (= (bit-xor 12 10) 6))          ; 1100 ^ 1010 = 0110
(assert (= (bit-not 0) -1))
(assert (= (bit-shl 1 4) 16))
(assert (= (bit-shr 16 2) 4))
(assert (= (bit-and 255 15 7) 7))
(assert (= (bit-or 1 2 4 8) 15))
