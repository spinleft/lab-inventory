import {
  ApartmentOutlined,
  BellOutlined,
  DatabaseOutlined,
  FieldTimeOutlined,
  LockOutlined,
  SafetyCertificateOutlined,
  SettingOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { Button, Card, Col, Row, Space, Statistic, Tag, Typography } from "antd";
import { Link } from "react-router-dom";
import { useAppShell } from "../../app/AppShell";
import {
  canAccessAdminSettings,
  describeRole,
  describeScope,
} from "../auth/permissions";

const { Paragraph, Text, Title } = Typography;

export function DashboardPage() {
  const { currentUser } = useAppShell();
  const quickActions = [
    {
      icon: <UserOutlined />,
      title: "用户资料",
      description: "查看当前账号、角色和数据范围。",
      to: "/settings/profile",
    },
    {
      icon: <LockOutlined />,
      title: "密码",
      description: "修改当前账号的登录密码。",
      to: "/settings/password",
    },
    {
      icon: <SettingOutlined />,
      title: "偏好设置",
      description: "进入用户偏好设置入口。",
      to: "/settings/preference",
    },
    ...(canAccessAdminSettings(currentUser)
      ? [
          {
            icon: <ApartmentOutlined />,
            title: "管理中心",
            description:
              currentUser.user_type.name === "maintainer"
                ? "管理自己实验室的用户和数据。"
                : "管理实验室、用户和系统数据。",
            to: "/admin",
          },
        ]
      : []),
  ];

  return (
    <Space orientation="vertical" size="large" className="full-width">
      <Card className="dashboard-hero-card">
        <div className="dashboard-hero">
          <div>
            <Title level={2} className="dashboard-hero-title">
              欢迎回来，{currentUser.username}
            </Title>
            <Paragraph className="dashboard-hero-copy">
              后台框架已经接入当前会话。你可以从左侧导航或右上角用户菜单进入权限范围内的设置页面。
            </Paragraph>
            <Space wrap>
              <Tag color="processing">{describeRole(currentUser)}</Tag>
              <Tag>{describeScope(currentUser)}</Tag>
            </Space>
          </div>
          <SafetyCertificateOutlined className="dashboard-hero-icon" aria-hidden="true" />
        </div>
      </Card>

      <Row gutter={[16, 16]}>
        <Col xs={24} md={8}>
          <Card className="dashboard-stat-card">
            <Statistic title="会话状态" value="已连接" prefix={<BellOutlined />} />
            <Text type="secondary">已通过后端会话校验。</Text>
          </Card>
        </Col>
        <Col xs={24} md={8}>
          <Card className="dashboard-stat-card">
            <Statistic
              title="实验室范围"
              value={describeScope(currentUser)}
              prefix={<ApartmentOutlined />}
            />
            <Text type="secondary">数据权限以服务器返回范围为准。</Text>
          </Card>
        </Col>
        <Col xs={24} md={8}>
          <Card className="dashboard-stat-card">
            <Statistic title="后台数据" value="待接入" prefix={<DatabaseOutlined />} />
            <Text type="secondary">库存、借用和维护统计将在后续接入。</Text>
          </Card>
        </Col>
      </Row>

      <Card title="快捷入口" className="dashboard-section-card">
        <Row gutter={[16, 16]}>
          {quickActions.map((action) => (
            <Col xs={24} lg={8} key={action.to}>
              <div className="dashboard-action-card">
                <Space align="start">
                  <span className="dashboard-action-icon">{action.icon}</span>
                  <div>
                    <Title level={5} className="dashboard-action-title">
                      {action.title}
                    </Title>
                    <Paragraph type="secondary">{action.description}</Paragraph>
                    <Link to={action.to}>
                      <Button type="link" className="dashboard-action-link">
                        打开
                      </Button>
                    </Link>
                  </div>
                </Space>
              </div>
            </Col>
          ))}
        </Row>
      </Card>

      <Card title="后续工作区" className="dashboard-section-card">
        <Row gutter={[16, 16]}>
          {["库存概览", "借用流程", "维护计划"].map((title) => (
            <Col xs={24} md={8} key={title}>
              <div className="dashboard-placeholder">
                <FieldTimeOutlined aria-hidden="true" />
                <Text>{title}</Text>
              </div>
            </Col>
          ))}
        </Row>
      </Card>
    </Space>
  );
}
