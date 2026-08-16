-- 用户备注。
--
-- 管理员用它记"这人是谁、为什么给这个权限";访客自助注册时也能自己填一句
-- 说明来意,审批的人不必再去问。可空,已有账号一律留空。
ALTER TABLE users ADD COLUMN description TEXT;
