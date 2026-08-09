import { DownOutlined } from "@ant-design/icons";
import {
  Avatar,
  Breadcrumb,
  Button,
  Dropdown,
  Layout,
  Menu,
  Typography,
  type MenuProps,
} from "antd";
import { type ReactNode } from "react";

const { Content, Header, Sider } = Layout;
const { Text } = Typography;

export type AppChromeNavItem = {
  key: string;
  icon?: ReactNode;
  label: ReactNode;
  disabled?: boolean;
};

export type AppChromeBreadcrumbItem = {
  key: string;
  label: ReactNode;
  onClick?: () => void;
};

type AppChromeProps = {
  brandLabel?: string;
  breadcrumbItems: AppChromeBreadcrumbItem[];
  children: ReactNode;
  isUserMenuLoading?: boolean;
  onBrandClick: () => void;
  onSidebarSelect: (key: string) => void;
  onUserMenuClick: NonNullable<MenuProps["onClick"]>;
  pageIcon?: ReactNode;
  pageMeta?: ReactNode;
  pageTitle: ReactNode;
  selectedSidebarKey?: string;
  sidebarItems: AppChromeNavItem[];
  sidebarTitle: string;
  userInitial: string;
  userMenuItems: MenuProps["items"];
  userName: string;
};

export function AppChrome({
  brandLabel = "Lab Inventory",
  breadcrumbItems,
  children,
  isUserMenuLoading = false,
  onBrandClick,
  onSidebarSelect,
  onUserMenuClick,
  pageIcon,
  pageMeta,
  pageTitle,
  selectedSidebarKey,
  sidebarItems,
  sidebarTitle,
  userInitial,
  userMenuItems,
  userName,
}: AppChromeProps) {
  return (
    <Layout className="app-shell">
      <Header className="app-shell-header">
        <button
          type="button"
          className="app-shell-brand"
          aria-label={`${brandLabel} 概览`}
          onClick={onBrandClick}
        >
          <span className="app-shell-brand-mark">LI</span>
          <span className="app-shell-brand-text">{brandLabel}</span>
        </button>

        <Dropdown
          placement="bottomRight"
          trigger={["click"]}
          menu={{
            items: userMenuItems,
            onClick: onUserMenuClick,
          }}
        >
          <Button type="text" className="app-user-menu" loading={isUserMenuLoading}>
            <Avatar size={28} className="app-user-avatar">
              {userInitial}
            </Avatar>
            <span className="app-user-name">{userName}</span>
            <DownOutlined aria-hidden="true" />
          </Button>
        </Dropdown>
      </Header>

      <Layout className="app-shell-body">
        <Sider
          breakpoint="md"
          collapsedWidth={0}
          theme="light"
          width={244}
          className="app-shell-sider"
        >
          <div className="app-sider-section">
            <Text type="secondary" className="app-sider-label">
              {sidebarTitle}
            </Text>
            <Menu
              mode="inline"
              selectedKeys={selectedSidebarKey ? [selectedSidebarKey] : []}
              items={sidebarItems}
              onClick={({ key }) => onSidebarSelect(key)}
            />
          </div>
        </Sider>

        <Layout className="app-shell-main">
          <Content className="app-shell-content">
            <Breadcrumb
              className="app-breadcrumb"
              items={breadcrumbItems.map((item) => ({
                title: item.onClick ? (
                  <button
                    type="button"
                    className="app-breadcrumb-link app-breadcrumb-button"
                    onClick={item.onClick}
                  >
                    {item.label}
                  </button>
                ) : (
                  item.label
                ),
              }))}
            />

            <div className="app-page-title-row">
              <div>
                <Typography.Title level={1} className="app-page-title">
                  {pageTitle}
                </Typography.Title>
                {pageMeta}
              </div>
              {pageIcon}
            </div>

            {children}
          </Content>
        </Layout>
      </Layout>
    </Layout>
  );
}
