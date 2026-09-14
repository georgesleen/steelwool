(define (render-status-line editor-state current-mode selection-count diagnostics)
(compose-status-line editor-state current-mode selection-count diagnostics (current-file-name) (current-line-number)))

(short-call a b)

(apply-transformation (build-pipeline (stage-one input-buffer) (stage-two input-buffer) (stage-three input-buffer)) output-buffer)
