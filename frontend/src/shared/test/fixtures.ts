export const testCurrentUser = {
  user_id: "00000000-0000-4000-8000-000000000001",
  username: "admin",
  email: "admin@example.com",
  user_type: {
    user_type_id: "00000000-0000-4000-8000-000000000002",
    name: "owner",
  },
  laboratory: null,
};

export const testMaintainerUser = {
  user_id: "00000000-0000-4000-8000-000000000011",
  username: "maintainer",
  email: "maintainer@example.com",
  user_type: {
    user_type_id: "00000000-0000-4000-8000-000000000012",
    name: "maintainer",
  },
  laboratory: {
    laboratory_id: "00000000-0000-4000-8000-000000000013",
    name: "化学实验室",
  },
};

export const testRegularUser = {
  user_id: "00000000-0000-4000-8000-000000000021",
  username: "lab-user",
  email: "lab-user@example.com",
  user_type: {
    user_type_id: "00000000-0000-4000-8000-000000000022",
    name: "user",
  },
  laboratory: {
    laboratory_id: "00000000-0000-4000-8000-000000000023",
    name: "材料实验室",
  },
};
