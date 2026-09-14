(define (classify value)
(cond [(and (number? value) (> value 100)) (let ([scaled (* value 2)] [label "big"]) (list label scaled))]
[(string? value) (let* ([trimmed (trim value)] [size (string-length trimmed)]) (when (> size 0) (list "string" size)))]
[else (lambda (fallback) (begin (log! "unclassified") (fallback value)))]))

(define (loop-forever n)
(let loop ([index 0] [accumulator '()])
(if (= index n) (reverse accumulator) (loop (+ index 1) (cons index accumulator)))))
