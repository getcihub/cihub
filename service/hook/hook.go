package hook

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/getcihub/cihub/core"
)

const workflowJobEvent = "workflow_job"

type service struct {
	secret string
}

func New(secret string) core.HookParser {
	return &service{secret}
}

func (s *service) Parse(r *http.Request) (*core.Hook, error) {
	defer func() {
		_, _ = io.Copy(io.Discard, r.Body)
		_ = r.Body.Close()
	}()

	if r.Method != http.MethodPost {
		return nil, fmt.Errorf("invalid HTTP Method")
	}

	event := r.Header.Get("X-GitHub-Event")
	if event == "" {
		return nil, fmt.Errorf("missing X-GitHub-Event Header")
	}

	if event != workflowJobEvent {
		return nil, nil
	}

	payload, err := io.ReadAll(r.Body)
	if err != nil || len(payload) == 0 {
		return nil, fmt.Errorf("error parsing payload")
	}

	if len(s.secret) > 0 {
		signature := r.Header.Get("X-Hub-Signature-256")
		if len(signature) == 0 {
			return nil, fmt.Errorf("missing X-Hub-Signature-256 Header")
		}

		signature = strings.TrimPrefix(signature, "sha256=")

		mac := hmac.New(sha256.New, []byte(s.secret))
		_, _ = mac.Write(payload)
		expectedMAC := hex.EncodeToString(mac.Sum(nil))

		if !hmac.Equal([]byte(signature), []byte(expectedMAC)) {
			return nil, fmt.Errorf("HMAC verification failed")
		}
	}

	var hook core.Hook
	if err := json.Unmarshal([]byte(payload), &hook); err != nil {
		return nil, err
	}

	return &hook, nil
}
