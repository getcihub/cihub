package metadata

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// defaultMMDSAddress is the address of firecracker's metadata service
const defaultMMDSAddress = "169.254.169.254"

type service struct {
	client *http.Client
}

type metadataResponse struct {
	Latest struct {
		Metadata struct {
			CIHub *core.Metadata `json:"cihub"`
		} `json:"meta-data"`
	} `json:"latest"`
}

func New() core.MetadataService {
	return &service{http.DefaultClient}
}

func (s *service) Find(ctx context.Context, path string) (*core.Metadata, error) {
	log := logger.FromContext(ctx).WithFields(
		logrus.Fields{
			"address": defaultMMDSAddress,
			"path":    path,
		},
	)

	token, err := s.generateToken(ctx)
	if err != nil {
		return nil, err
	}

	req, err := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("http://%s/latest/metadata/%s", defaultMMDSAddress, strings.TrimPrefix(path, "/")), nil)
	if err != nil {
		log.WithError(err).
			Warnln("mmds: cannot get metadata")
		return nil, err
	}

	req.Header.Set("Accept", "application/json")
	req.Header.Set("X-Metadata-Token", token)

	res, err := s.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()

	if res.StatusCode >= 300 {
		body, _ := io.ReadAll(res.Body)
		log.WithField("status", res.StatusCode).
			Warnln("mmds: unexpected response status")
		return nil, fmt.Errorf("metadata: unexpected status %d: %s", res.StatusCode, string(body))
	}

	var md metadataResponse
	err = json.NewDecoder(res.Body).Decode(&md)
	if err != nil {
		log.WithError(err).
			Warnln("mmds: failed to decode response")
		return nil, fmt.Errorf("mmds: cannot decode response, err: %w", err)
	}

	return md.Latest.Metadata.CIHub, nil
}

func (s *service) generateToken(ctx context.Context) (string, error) {
	req, err := http.NewRequestWithContext(ctx, "PUT", fmt.Sprintf("http://%s/latest/api/token", defaultMMDSAddress), nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("X-Metadata-Token-TTL-Seconds", "21600")

	res, err := s.client.Do(req)
	if err != nil {
		return "", err
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusOK {
		return "", fmt.Errorf("mmds: unexpected status code: %s %s: %d", req.Method, req.URL, res.StatusCode)
	}

	token, err := io.ReadAll(res.Body)
	if err != nil {
		return "", err
	}

	return string(token), nil
}
