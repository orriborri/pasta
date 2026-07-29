# kb-engine AWS Deployment

## Architecture

```
┌─────────────────────────────────────────────────────┐
│ ECS Fargate                                          │
│   kb serve --port 3030                               │
│   Mounts: EFS → /data (vectors + index + state.db)  │
│   Env: OPENAI_API_KEY from Secrets Manager           │
└────────────────────┬────────────────────────────────┘
                     │
     ┌───────────────┼───────────────┐
     │               │               │
┌────▼───┐    ┌─────▼─────┐   ┌────▼────────┐
│   S3    │    │    EFS     │   │  Secrets    │
│ Parquet │    │  LanceDB   │   │  Manager    │
│ (raw/)  │    │  Tantivy   │   │  API keys   │
│         │    │  state.db  │   │             │
└─────────┘    └───────────┘   └─────────────┘
```

## Resources

### S3 Bucket — Parquet storage (source of truth)
```
Bucket: readpeak-kb-{environment}
Path:   raw/{source}/{year-month}/{timestamp}.parquet
Lifecycle: IA after 90 days
```

### EFS — Derived indexes (fast access required)
```
FileSystem: kb-data-{environment}
Mount:      /data
Contents:   vectors/ (LanceDB), index/ (Tantivy), state.db
Throughput: Bursting (sufficient for single-user)
```

### Secrets Manager
```
Secret: kb-engine/{environment}/api-keys
Keys:
  OPENAI_API_KEY: text-embedding-3-small key
  SLACK_TOKEN: (if fetching from Slack in cloud)
```

## ECS Task Definition

```json
{
  "family": "kb-engine",
  "cpu": "512",
  "memory": "1024",
  "networkMode": "awsvpc",
  "containerDefinitions": [{
    "name": "kb",
    "image": "{account}.dkr.ecr.{region}.amazonaws.com/kb-engine:latest",
    "portMappings": [{"containerPort": 3030}],
    "mountPoints": [{"sourceVolume": "kb-data", "containerPath": "/data"}],
    "environment": [
      {"name": "KB_DATA_DIR", "value": "/data"},
      {"name": "RUST_LOG", "value": "info"}
    ],
    "secrets": [
      {"name": "OPENAI_API_KEY", "valueFrom": "arn:aws:secretsmanager:{region}:{account}:secret:kb-engine/{env}/api-keys:OPENAI_API_KEY::"}
    ],
    "logConfiguration": {
      "logDriver": "awslogs",
      "options": {
        "awslogs-group": "/ecs/kb-engine",
        "awslogs-region": "{region}",
        "awslogs-stream-prefix": "kb"
      }
    }
  }],
  "volumes": [{
    "name": "kb-data",
    "efsVolumeConfiguration": {
      "fileSystemId": "{efs-id}",
      "transitEncryption": "ENABLED"
    }
  }]
}
```

## Estimated Cost (single-user)

| Resource | Monthly |
|----------|---------|
| ECS Fargate (0.5 vCPU, 1GB, always-on) | ~$18 |
| EFS (5GB, bursting) | ~$1.50 |
| S3 (1GB Parquet) | ~$0.03 |
| Secrets Manager (2 secrets) | ~$0.80 |
| CloudWatch Logs | ~$1 |
| **Total** | **~$21/month** |

## Sync Strategy (Cloud)

Option A: Cron-triggered ECS task runs `kb sync` every hour
Option B: Always-on service with internal scheduler (current approach)

For single-user: Option B (simpler, always available for search).
For team: Option A + ALB in front of `kb serve`.
