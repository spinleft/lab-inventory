import { z } from "zod";

export const currentUserSchema = z.object({
  user_id: z.string().uuid(),
  username: z.string(),
  email: z.string().nullable().optional(),
  user_type: z.object({
    user_type_id: z.string().uuid(),
    name: z.string(),
  }),
  laboratory: z
    .object({
      laboratory_id: z.string().uuid(),
      name: z.string(),
    })
    .nullable(),
});

export type CurrentUser = z.infer<typeof currentUserSchema>;
