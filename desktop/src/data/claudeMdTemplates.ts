/**
 * CLAUDE.md Templates
 *
 * Pre-defined templates for common CLAUDE.md patterns.
 */

export interface ClaudeMdTemplate {
  id: string;
  name: string;
  description: string;
  content: string;
}

export const claudeMdTemplates: ClaudeMdTemplate[] = [
  {
    id: 'basic',
    name: 'Basic',
    description: 'A simple project overview template',
    content: `# Project Name

## Overview

Brief description of what this project does.

## Getting Started

### Prerequisites

- Requirement 1
- Requirement 2

### Installation

\`\`\`bash
# Installation commands
npm install
\`\`\`

### Running

\`\`\`bash
npm start
\`\`\`

## Project Structure

\`\`\`
src/
  components/   # React components
  utils/        # Utility functions
  types/        # TypeScript types
\`\`\`

## Contributing

Guidelines for contributing to this project.

## License

MIT
`,
  },
  {
    id: 'api-docs',
    name: 'API Documentation',
    description: 'Template for documenting APIs and endpoints',
    content: `# API Documentation

## Base URL

\`\`\`
https://api.example.com/v1
\`\`\`

## Authentication

All API requests require authentication using a Bearer token:

\`\`\`
Authorization: Bearer <your-api-token>
\`\`\`

## Endpoints

### GET /resource

Retrieves a list of resources.

**Parameters:**

| Parameter | Type   | Required | Description           |
|-----------|--------|----------|----------------------|
| limit     | number | No       | Max items to return  |
| offset    | number | No       | Pagination offset    |

**Response:**

\`\`\`json
{
  "data": [...],
  "total": 100,
  "limit": 10,
  "offset": 0
}
\`\`\`

### POST /resource

Creates a new resource.

**Request Body:**

\`\`\`json
{
  "name": "string",
  "description": "string"
}
\`\`\`

**Response:**

\`\`\`json
{
  "id": "123",
  "name": "string",
  "created_at": "2024-01-01T00:00:00Z"
}
\`\`\`

## Error Codes

| Code | Description            |
|------|------------------------|
| 400  | Bad Request           |
| 401  | Unauthorized          |
| 403  | Forbidden             |
| 404  | Not Found             |
| 500  | Internal Server Error |

## Rate Limiting

- 100 requests per minute per API key
- Rate limit headers included in all responses
`,
  },
  {
    id: 'coding-guidelines',
    name: 'Coding Guidelines',
    description: 'Template for coding standards and best practices',
    content: `# Coding Guidelines

## Code Style

### General Principles

- Write clean, readable, and maintainable code
- Follow the DRY (Don't Repeat Yourself) principle
- Keep functions small and focused on a single task
- Use meaningful variable and function names

### Naming Conventions

- **Variables**: camelCase (\`userData\`, \`isActive\`)
- **Constants**: UPPER_SNAKE_CASE (\`MAX_RETRIES\`, \`API_BASE_URL\`)
- **Functions**: camelCase, verb prefix (\`getUserById\`, \`validateInput\`)
- **Classes/Types**: PascalCase (\`UserService\`, \`ConfigOptions\`)
- **Files**: kebab-case (\`user-service.ts\`, \`api-client.ts\`)

### TypeScript Guidelines

\`\`\`typescript
// Prefer interfaces for object shapes
interface User {
  id: string;
  name: string;
  email: string;
}

// Use type for unions and intersections
type Status = 'pending' | 'active' | 'completed';

// Always type function parameters and returns
function processUser(user: User): ProcessedUser {
  // ...
}
\`\`\`

## Error Handling

- Always handle errors explicitly
- Use custom error classes for domain-specific errors
- Log errors with sufficient context
- Never swallow exceptions silently

\`\`\`typescript
try {
  const result = await riskyOperation();
} catch (error) {
  logger.error('Operation failed', { error, context });
  throw new AppError('Failed to process', { cause: error });
}
\`\`\`

## Testing

- Write tests for all new features
- Aim for >80% code coverage
- Use descriptive test names
- Follow AAA pattern (Arrange, Act, Assert)

\`\`\`typescript
describe('UserService', () => {
  it('should return user by ID when user exists', async () => {
    // Arrange
    const userId = '123';

    // Act
    const result = await userService.getById(userId);

    // Assert
    expect(result).toMatchObject({ id: userId });
  });
});
\`\`\`

## Git Workflow

- Use conventional commits: \`feat:\`, \`fix:\`, \`docs:\`, \`refactor:\`
- Create feature branches from \`main\`
- Keep commits atomic and well-described
- Squash WIP commits before merging
`,
  },
  {
    id: 'project-structure',
    name: 'Project Structure',
    description: 'Template for documenting project architecture',
    content: `# Project Structure

## Directory Layout

\`\`\`
project-root/
├── src/
│   ├── components/     # React/UI components
│   │   ├── common/     # Shared/reusable components
│   │   ├── features/   # Feature-specific components
│   │   └── layouts/    # Layout components
│   │
│   ├── services/       # Business logic and API clients
│   │   ├── api/        # API client implementations
│   │   └── domain/     # Domain services
│   │
│   ├── store/          # State management
│   │   ├── slices/     # Redux slices or Zustand stores
│   │   └── hooks/      # Custom state hooks
│   │
│   ├── types/          # TypeScript type definitions
│   │   ├── api.ts      # API response types
│   │   ├── domain.ts   # Domain model types
│   │   └── index.ts    # Type exports
│   │
│   ├── utils/          # Utility functions
│   │   ├── formatters/ # Data formatting
│   │   ├── validators/ # Input validation
│   │   └── helpers/    # Misc helpers
│   │
│   ├── hooks/          # Custom React hooks
│   ├── constants/      # Application constants
│   └── config/         # Configuration files
│
├── tests/              # Test files
│   ├── unit/           # Unit tests
│   ├── integration/    # Integration tests
│   └── e2e/            # End-to-end tests
│
├── docs/               # Documentation
├── scripts/            # Build and utility scripts
└── public/             # Static assets
\`\`\`

## Key Files

| File | Purpose |
|------|---------|
| \`src/main.tsx\` | Application entry point |
| \`src/App.tsx\` | Root component |
| \`src/config/index.ts\` | Environment configuration |
| \`vite.config.ts\` | Build configuration |
| \`tsconfig.json\` | TypeScript configuration |

## Module Dependencies

\`\`\`
components -> services -> types
     |            |
     v            v
   hooks <----- store
\`\`\`

## Adding New Features

1. Create types in \`src/types/\`
2. Add services in \`src/services/\`
3. Create store slice in \`src/store/\`
4. Build components in \`src/components/features/\`
5. Add routes if needed
6. Write tests in \`tests/\`
`,
  },
  {
    id: 'troubleshooting',
    name: 'Troubleshooting',
    description: 'Template for common issues and solutions',
    content: `# Troubleshooting Guide

## Common Issues

### Build Failures

#### Issue: Module not found errors

**Symptoms:**
\`\`\`
Error: Cannot find module './component'
\`\`\`

**Solution:**
1. Check the import path is correct
2. Verify the file exists
3. Clear the build cache: \`rm -rf node_modules/.cache\`
4. Reinstall dependencies: \`rm -rf node_modules && npm install\`

---

#### Issue: TypeScript compilation errors

**Symptoms:**
\`\`\`
error TS2307: Cannot find module
\`\`\`

**Solution:**
1. Run \`npm run typecheck\` to see all errors
2. Check tsconfig.json paths configuration
3. Ensure @types packages are installed

---

### Runtime Errors

#### Issue: API connection failures

**Symptoms:**
- Network request timeouts
- CORS errors in browser console

**Solution:**
1. Verify API server is running
2. Check CORS configuration on server
3. Ensure correct environment variables are set
4. Check network/firewall settings

---

#### Issue: State not updating

**Symptoms:**
- UI doesn't reflect expected changes
- Console shows stale data

**Solution:**
1. Verify mutation/action is being called
2. Check for immutability issues
3. Ensure component is subscribed to state
4. Use React DevTools to inspect state

---

## Debug Commands

| Command | Purpose |
|---------|---------|
| \`npm run dev -- --debug\` | Start with debug logging |
| \`npm run typecheck\` | Check TypeScript errors |
| \`npm run lint\` | Run linter |
| \`npm run test -- --watch\` | Run tests in watch mode |

## Getting Help

- Check existing GitHub issues
- Search the documentation
- Ask in #dev-support channel
- File a new issue with reproduction steps

## Reporting Bugs

Include the following information:
1. Steps to reproduce
2. Expected behavior
3. Actual behavior
4. Environment details (OS, Node version, etc.)
5. Relevant logs/screenshots
`,
  },
];

export default claudeMdTemplates;
