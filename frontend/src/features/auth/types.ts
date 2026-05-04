import { z } from "zod";

export const currentUserSchema = z.object({
  user_id: z.string(),
  username: z.string(),
  email: z.string().nullable(),
  user_type: z.object({
    user_type_id: z.string(),
    name: z.string(),
  }),
  laboratory: z
    .object({
      laboratory_id: z.string(),
      name: z.string(),
    })
    .nullable(),
});

export type CurrentUser = z.infer<typeof currentUserSchema>;
