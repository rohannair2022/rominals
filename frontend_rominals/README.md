## Frontend

This is the Next.js frontend for **Rominals**. It was scaffolded with the App Router, TypeScript, Tailwind CSS, and ESLint so it can grow into the UI for the Rust market-data backend.

## Local development

Install dependencies and run the development server:

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

The main app entry points are in `src/app/`.

## Available scripts

- `npm run dev` - start the local development server
- `npm run build` - create a production build
- `npm run start` - run the production server
- `npm run lint` - run ESLint

## Next steps

Typical follow-up work:

- connect the frontend to the Rust backend through route handlers or direct API calls
- add pages for quote lookup, watchlists, and portfolio tracking
- introduce shared UI components under `src/`
